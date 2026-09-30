//! Transfer paths share accounting, throttling and rendering. Kernel calls never
//! restart a transfer: after any successful call the descriptors already point
//! at the remaining data, so fallback continues from those offsets.
use crate::cli::Config;
use crate::display::Display;
use std::fs::{File, OpenOptions};
use std::io::{self, ErrorKind, Read, Seek, SeekFrom, Write};
#[cfg(unix)]
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::time::{Duration, Instant};

pub enum Input {
    File(File),
    #[cfg(not(unix))]
    Stdin(io::Stdin),
}

impl Input {
    pub fn open(path: &str) -> io::Result<Self> {
        if path == "-" || path == "/dev/stdin" {
            #[cfg(unix)]
            {
                return duplicate(0).map(Self::File);
            }
            #[cfg(not(unix))]
            {
                return Ok(Self::Stdin(io::stdin()));
            }
        }
        File::open(path).map(Self::File)
    }
    pub fn regular(&self) -> bool {
        match self {
            Self::File(f) => f.metadata().is_ok_and(|m| m.is_file()),
            #[cfg(not(unix))]
            Self::Stdin(_) => false,
        }
    }
    pub fn estimate(&mut self, line: bool, delim: u8) -> io::Result<Option<u64>> {
        if !self.regular() {
            return Ok(None);
        }
        let f = match self {
            Self::File(f) => f,
            #[cfg(not(unix))]
            Self::Stdin(_) => return Ok(None),
        };
        let position = f.stream_position()?;
        if !line {
            return Ok(Some(f.metadata()?.len().saturating_sub(position)));
        }
        let mut count = 0u64;
        let mut buffer = vec![0; 128 * 1024];
        let result = (|| {
            loop {
                let n = match f.read(&mut buffer) {
                    Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                    other => other?,
                };
                if n == 0 {
                    break;
                }
                count = count
                    .checked_add(memchr::memchr_iter(delim, &buffer[..n]).count() as u64)
                    .ok_or_else(|| io::Error::other("size overflow"))?;
            }
            Ok(Some(count))
        })();
        f.seek(SeekFrom::Start(position))?;
        result
    }
    #[cfg(unix)]
    pub fn fd(&self) -> RawFd {
        match self {
            Self::File(f) => f.as_raw_fd(),
        }
    }
    fn skip_error(&mut self, block: u64, remaining: Option<u64>) -> io::Result<usize> {
        let f = match self {
            Self::File(f) => f,
            #[cfg(not(unix))]
            Self::Stdin(_) => {
                return Err(io::Error::other(
                    "cannot recover read errors on non-seekable input",
                ))
            }
        };
        if !f.metadata()?.is_file() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::FileTypeExt;
                if !f.metadata()?.file_type().is_block_device() {
                    return Err(io::Error::other(
                        "cannot recover read errors on non-seekable input",
                    ));
                }
            }
            #[cfg(not(unix))]
            {
                return Err(io::Error::other(
                    "cannot recover read errors on non-seekable input",
                ));
            }
        }
        let pos = f.stream_position()?;
        let skip = block - pos % block;
        let mut skip = skip.min(remaining.unwrap_or(u64::MAX));
        if f.metadata()?.is_file() {
            skip = skip.min(f.metadata()?.len().saturating_sub(pos));
        }
        f.seek(SeekFrom::Current(
            i64::try_from(skip).map_err(io::Error::other)?,
        ))?;
        usize::try_from(skip).map_err(io::Error::other)
    }
}
impl Read for Input {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::File(f) => f.read(b),
            #[cfg(not(unix))]
            Self::Stdin(s) => s.read(b),
        }
    }
}

pub enum Output {
    File(File),
    #[cfg(not(unix))]
    Stdout(io::Stdout),
    #[cfg(not(unix))]
    Discard,
}

impl Output {
    pub fn open(config: &Config) -> io::Result<Self> {
        if config.discard {
            #[cfg(unix)]
            {
                return OpenOptions::new()
                    .write(true)
                    .open("/dev/null")
                    .map(Self::File);
            }
            #[cfg(not(unix))]
            {
                return Ok(Self::Discard);
            }
        }
        if let Some(path) = &config.output {
            return OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(path)
                .map(Self::File)
                .map_err(|e| {
                    io::Error::new(
                        e.kind(),
                        format!("failed to create output file '{path}': {e}"),
                    )
                });
        }
        #[cfg(unix)]
        {
            duplicate(1).map(Self::File)
        }
        #[cfg(not(unix))]
        {
            Ok(Self::Stdout(io::stdout()))
        }
    }
    pub fn regular(&self) -> bool {
        match self {
            Self::File(f) => f.metadata().is_ok_and(|m| m.is_file()),
            #[cfg(not(unix))]
            _ => false,
        }
    }
    #[cfg(unix)]
    pub fn fd(&self) -> Option<RawFd> {
        match self {
            Self::File(f) => Some(f.as_raw_fd()),
        }
    }
    pub fn pipe(&self) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::fs::FileTypeExt;
            match self {
                Self::File(f) => f
                    .metadata()
                    .is_ok_and(|m| m.file_type().is_fifo() || m.file_type().is_socket()),
            }
        }
        #[cfg(not(unix))]
        {
            false
        }
    }
    pub fn sync(&self) -> io::Result<()> {
        match self {
            Self::File(f) if self.regular() => f.sync_data(),
            _ => Ok(()),
        }
    }
    pub fn hole(&mut self, n: usize) -> io::Result<bool> {
        if !self.regular() {
            return Ok(false);
        }
        match self {
            Self::File(f) => {
                f.seek(SeekFrom::Current(n as i64))?;
                Ok(true)
            }
            #[cfg(not(unix))]
            _ => Ok(false),
        }
    }
    pub fn finish(&mut self, sparse: bool) -> io::Result<()> {
        if sparse && self.regular() {
            match self {
                Self::File(f) => {
                    let n = f.stream_position()?;
                    if n > f.metadata()?.len() {
                        f.set_len(n)?;
                    }
                }
                #[cfg(not(unix))]
                _ => {}
            }
        }
        flush_checked(self)
    }
}
impl Write for Output {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        match self {
            Self::File(f) => f.write(b),
            #[cfg(not(unix))]
            Self::Stdout(s) => s.write(b),
            #[cfg(not(unix))]
            Self::Discard => Ok(b.len()),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::File(f) => f.flush(),
            #[cfg(not(unix))]
            Self::Stdout(s) => s.flush(),
            #[cfg(not(unix))]
            Self::Discard => Ok(()),
        }
    }
}

#[cfg(unix)]
fn duplicate(fd: RawFd) -> io::Result<File> {
    // SAFETY: dup creates a new owned descriptor; File closes only that duplicate.
    let fd = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 3) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

/// Before truncating any output, reject aliases (including hard links) of inputs.
pub fn validate_paths(config: &Config) -> io::Result<()> {
    let outputs = [
        config.output.as_deref(),
        config.store_and_forward.as_deref().filter(|p| *p != "-"),
        config.pidfile.as_deref(),
    ];
    for (i, out) in outputs.iter().enumerate() {
        let Some(out) = out else {
            continue;
        };
        for input in &config.files {
            if same_file(input, out)? {
                return Err(io::Error::other(format!(
                    "input and output refer to the same file: {out}"
                )));
            }
        }
        #[cfg(unix)]
        {
            let stdin = Input::open("-")?;
            let Input::File(f) = stdin;
            {
                if let Ok(m) = std::fs::metadata(out) {
                    use std::os::unix::fs::MetadataExt;
                    let input = f.metadata()?;
                    if input.is_file() && input.dev() == m.dev() && input.ino() == m.ino() {
                        return Err(io::Error::other("output aliases standard input"));
                    }
                }
            }
        }
        for other in outputs[..i].iter().flatten() {
            if same_file(out, other)? {
                return Err(io::Error::other(
                    "output, spool and PID files must be distinct",
                ));
            }
        }
    }
    Ok(())
}

fn same_file(a: &str, b: &str) -> io::Result<bool> {
    if a == b {
        return Ok(true);
    }
    let (Ok(a), Ok(b)) = (std::fs::metadata(a), std::fs::metadata(b)) else {
        return Ok(false);
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(a.dev() == b.dev() && a.ino() == b.ino())
    }
    #[cfg(not(unix))]
    {
        let _ = (a, b);
        Ok(false)
    }
}

/// Polling keeps elapsed time and rate output alive while either end stalls.
#[cfg(unix)]
pub fn wait_ready(fd: RawFd, events: i16, display: &mut Display) -> io::Result<()> {
    loop {
        if display
            .cancel
            .as_ref()
            .is_some_and(|c| c.load(std::sync::atomic::Ordering::Relaxed))
        {
            return Ok(());
        }
        let mut poll = libc::pollfd {
            fd,
            events,
            revents: 0,
        };
        let millis = (display.config.interval * 1000.0).clamp(1.0, 100.0) as i32;
        // SAFETY: poll points to exactly one initialized pollfd for this call.
        let result = unsafe { libc::poll(&mut poll, 1, millis) };
        display.tick(false)?;
        if result > 0 {
            return Ok(());
        }
        if result < 0 && io::Error::last_os_error().kind() != ErrorKind::Interrupted {
            return Err(io::Error::last_os_error());
        }
    }
}

pub struct Throttle {
    start: Instant,
    units: u64,
    first: bool,
}
impl Default for Throttle {
    fn default() -> Self {
        Self {
            start: Instant::now(),
            units: 0,
            first: true,
        }
    }
}
impl Throttle {
    pub fn before(&mut self, amount: u64, display: &mut Display) -> io::Result<()> {
        if self.first {
            self.start = Instant::now();
            self.first = false;
        }
        self.units = self
            .units
            .checked_add(amount)
            .ok_or_else(|| io::Error::other("transfer counter overflow"))?;
        if display.config.limit == 0 {
            return Ok(());
        }
        let initial_rate = display.config.limit;
        while display.config.limit > 0 {
            // A remote update can release a throttled transfer immediately.
            if display.config.limit != initial_rate {
                self.start = Instant::now();
                self.units = amount;
                break;
            }
            let remaining = self.units as f64 / display.config.limit as f64
                - self.start.elapsed().as_secs_f64();
            if remaining <= 0.0 {
                break;
            }
            std::thread::sleep(Duration::from_secs_f64(remaining.clamp(0.0, 0.05)));
            display.tick(false)?;
        }
        Ok(())
    }
}

pub fn buffered(
    input: &mut Input,
    output: &mut Output,
    display: &mut Display,
    throttle: &mut Throttle,
) -> io::Result<()> {
    display.method = "read/write";
    #[cfg(target_os = "linux")]
    if let (Some(size), Some(fd)) = (display.config.pipe_buffer, output.fd()) {
        // -J is independent of the transfer backend. A denied or unsupported
        // capacity change is intentionally ignored, matching upstream.
        unsafe {
            libc::fcntl(fd, libc::F_SETPIPE_SZ, size);
        }
    }
    let mut storage = AlignedBuffer::new(display.config.buffer)?;
    let buffer_size = if display.config.direct_io {
        storage.layout.size()
    } else {
        display.config.buffer
    };
    let buffer = &mut storage.slice()[..buffer_size];
    let delim = if display.config.null { 0 } else { b'\n' };
    #[cfg(unix)]
    let input_regular = input.regular();
    let mut reported_error = false;
    let mut consecutive_errors = 0u64;
    loop {
        let remaining = if display.config.stop_at_size {
            display
                .config
                .total
                .map(|n| n.saturating_sub(display.units))
        } else {
            None
        };
        if remaining == Some(0) {
            break;
        }
        let mut max_read = buffer.len();
        if !display.config.line_mode {
            if let Some(n) = remaining {
                max_read = max_read.min(n.min(usize::MAX as u64) as usize);
            }
            if display.config.limit > 0 {
                max_read = max_read
                    .min((display.config.limit / 10).max(1).min(usize::MAX as u64) as usize);
            }
        } else if remaining.is_some() {
            // A line stop must not consume data belonging to the next pipeline
            // stage. Read one byte at a time only for this explicitly bounded mode.
            max_read = 1;
        }
        #[cfg(unix)]
        if !input_regular {
            wait_ready(input.fd(), libc::POLLIN, display)?;
        }
        if display
            .cancel
            .as_ref()
            .is_some_and(|c| c.load(std::sync::atomic::Ordering::Relaxed))
        {
            break;
        }
        #[cfg(target_os = "linux")]
        let read = if display.config.direct_io && input_regular {
            direct_tail(input.fd(), buffer.as_ptr(), max_read, || {
                read_retry(input, &mut buffer[..max_read])
            })
        } else {
            read_retry(input, &mut buffer[..max_read])
        };
        #[cfg(not(target_os = "linux"))]
        let read = read_retry(input, &mut buffer[..max_read]);
        let n = match read {
            Ok(0) => break,
            Ok(n) => {
                consecutive_errors = 0;
                n
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) if display.config.skip_errors > 0 => {
                if display.config.skip_errors == 1 || !reported_error {
                    eprintln!("pv: read error (zero-filling skipped range): {e}");
                    reported_error = true;
                }
                consecutive_errors += 1;
                let block = display
                    .config
                    .skip_block
                    .unwrap_or(if consecutive_errors > 5 { 512 } else { 1 });
                let n = input
                    .skip_error(
                        block,
                        if display.config.line_mode && delim != 0 {
                            None
                        } else {
                            remaining
                        },
                    )
                    .map_err(|e| crate::error::io_failure(8, "read error recovery failed", e))?;
                if n == 0 {
                    break;
                }
                // Recovery may skip a block larger than the transfer buffer.
                let mut left = n;
                while left > 0 {
                    let chunk = left.min(buffer.len());
                    buffer[..chunk].fill(0);
                    let units = if display.config.line_mode {
                        if delim == 0 {
                            chunk as u64
                        } else {
                            0
                        }
                    } else {
                        chunk as u64
                    };
                    throttle.before(units, display)?;
                    write_payload(output, &buffer[..chunk], units, display)?;
                    left -= chunk;
                }
                continue;
            }
            Err(e) => return Err(crate::error::io_failure(8, "read error", e)),
        };
        let data = &buffer[..n];
        if display.config.line_mode && display.config.limit > 0 {
            // Pace complete lines; long unterminated records still stream.
            let mut start = 0;
            for end in memchr::memchr_iter(delim, data) {
                throttle.before(1, display)?;
                write_payload(output, &data[start..=end], 1, display)?;
                start = end + 1;
            }
            if start < n {
                write_payload(output, &data[start..], 0, display)?;
            }
        } else {
            let units = if display.config.line_mode {
                memchr::memchr_iter(delim, data).count() as u64
            } else {
                n as u64
            };
            throttle.before(units, display)?;
            write_payload(output, data, units, display)?;
        }
    }
    Ok(())
}

fn write_payload(
    output: &mut Output,
    data: &[u8],
    units: u64,
    display: &mut Display,
) -> io::Result<()> {
    display.buffer_used = data.len();
    if display.config.sparse && data.iter().all(|b| *b == 0) && output.hole(data.len())? {
        display.record(data.len() as u64, units, Some(data));
    } else {
        let mut left = data;
        let output_pipe = output.pipe();
        while !left.is_empty() {
            #[cfg(unix)]
            if let Some(fd) = output.fd() {
                if output_pipe {
                    wait_ready(fd, libc::POLLOUT, display)?;
                }
            }
            // PIPE_BUF-sized writes cannot block halfway after writable polling.
            let max = if output_pipe {
                left.len().min(4096)
            } else {
                left.len()
            };
            #[cfg(target_os = "linux")]
            let write = if display.config.direct_io && output.regular() {
                direct_tail(output.fd().unwrap(), left.as_ptr(), max, || {
                    write_retry(output, &left[..max])
                })
            } else {
                write_retry(output, &left[..max])
            };
            #[cfg(not(target_os = "linux"))]
            let write = write_retry(output, &left[..max]);
            let n = match write {
                Ok(0) => {
                    return Err(crate::error::io_failure(
                        16,
                        "write error",
                        io::Error::new(ErrorKind::WriteZero, "no progress"),
                    ))
                }
                Ok(n) => n,
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(crate::error::io_failure(16, "write error", e)),
            };
            let count = if display.config.line_mode {
                memchr::memchr_iter(if display.config.null { 0 } else { b'\n' }, &left[..n]).count()
                    as u64
            } else {
                n as u64
            };
            display.record(n as u64, count, Some(&left[..n]));
            left = &left[n..];
            display.buffer_used = left.len();
            display.tick(false)?;
        }
    }
    if display.config.sync {
        output.sync()?;
    }
    display.buffer_used = 0;
    display.tick(false)
}

#[cfg(target_os = "linux")]
pub fn kernel(
    input: &mut Input,
    output: &mut Output,
    display: &mut Display,
    throttle: &mut Throttle,
) -> io::Result<bool> {
    use std::os::unix::fs::FileTypeExt;
    if display.config.no_splice || display.config.skip_errors > 0 {
        return Ok(false);
    }
    let Some(out_fd) = output.fd() else {
        return Ok(false);
    };
    let in_fd = input.fd();
    let in_regular = input.regular();
    let out_regular = output.regular();
    let in_pipe = match input {
        Input::File(f) => f.metadata()?.file_type().is_fifo(),
    };
    let out_pipe = match output {
        Output::File(f) => f.metadata()?.file_type().is_fifo(),
    };
    let mut intermediate = None;
    if !(in_regular && out_regular) && !in_pipe && !out_pipe {
        let mut fds = [0; 2];
        // SAFETY: pipe2 initializes both descriptor slots on success.
        if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } < 0 {
            return Ok(false);
        }
        intermediate = Some(unsafe { (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) });
    }
    if let Some(size) = display.config.pipe_buffer {
        // Best effort: upstream silently ignores permissions and kernel caps.
        for fd in [
            Some(out_fd),
            intermediate.as_ref().map(|(_, w)| w.as_raw_fd()),
        ]
        .into_iter()
        .flatten()
        {
            unsafe {
                libc::fcntl(fd, libc::F_SETPIPE_SZ, size);
            }
        }
    }
    loop {
        if display.config.no_splice {
            return Ok(false);
        }
        let remaining = if display.config.stop_at_size {
            display
                .config
                .total
                .map(|n| n.saturating_sub(display.units))
                .unwrap_or(u64::MAX)
        } else {
            u64::MAX
        };
        if remaining == 0 {
            return Ok(true);
        }
        let mut amount = remaining.min(1024 * 1024) as usize;
        if display.config.limit > 0 {
            amount = amount.min((display.config.limit / 10).max(1).min(usize::MAX as u64) as usize);
        }
        // Try the nonblocking splice first. Poll only on backpressure rather
        // than adding two poll syscalls to every successful pipe transfer.
        let result = if in_regular && out_regular {
            display.method = "copy_file_range";
            // SAFETY: live descriptors and null offset pointers use their current offsets.
            unsafe {
                libc::copy_file_range(
                    in_fd,
                    std::ptr::null_mut(),
                    out_fd,
                    std::ptr::null_mut(),
                    amount,
                    0,
                )
            }
        } else {
            display.method = "splice";
            let destination = intermediate
                .as_ref()
                .map(|(_, w)| w.as_raw_fd())
                .unwrap_or(out_fd);
            unsafe {
                libc::splice(
                    in_fd,
                    std::ptr::null_mut(),
                    destination,
                    std::ptr::null_mut(),
                    amount,
                    libc::SPLICE_F_MOVE | libc::SPLICE_F_NONBLOCK,
                )
            }
        };
        if result < 0 {
            let e = io::Error::last_os_error();
            if e.kind() == ErrorKind::Interrupted {
                continue;
            }
            if e.kind() == ErrorKind::WouldBlock {
                if in_pipe {
                    wait_ready(in_fd, libc::POLLIN, display)?;
                }
                if out_pipe {
                    wait_ready(out_fd, libc::POLLOUT, display)?;
                }
                continue;
            }
            if matches!(
                e.raw_os_error(),
                Some(libc::EINVAL | libc::ENOSYS | libc::EXDEV | libc::EOPNOTSUPP | libc::EBADF)
            ) {
                return Ok(false);
            }
            return Err(crate::error::io_failure(
                if e.raw_os_error() == Some(libc::EIO) {
                    8
                } else {
                    16
                },
                "kernel transfer error",
                e,
            ));
        }
        if result == 0 {
            return Ok(true);
        }
        let n = result as usize;
        if let Some((read, _)) = &intermediate {
            throttle.before(n as u64, display)?;
            let mut left = n;
            while left > 0 {
                let written = unsafe {
                    libc::splice(
                        read.as_raw_fd(),
                        std::ptr::null_mut(),
                        out_fd,
                        std::ptr::null_mut(),
                        left,
                        libc::SPLICE_F_MOVE,
                    )
                };
                if written < 0 {
                    let e = io::Error::last_os_error();
                    if e.kind() == ErrorKind::Interrupted {
                        continue;
                    }
                    if matches!(
                        e.raw_os_error(),
                        Some(libc::EINVAL | libc::ENOSYS | libc::EOPNOTSUPP)
                    ) {
                        // Input has already advanced. Drain staged bytes with read/write
                        // before falling back, rather than losing or duplicating them.
                        let mut read = read;
                        let mut staged = vec![0; left];
                        read.read_exact(&mut staged)
                            .map_err(|e| crate::error::io_failure(8, "read error", e))?;
                        write_payload(output, &staged, left as u64, display)?;
                        return Ok(false);
                    }
                    return Err(crate::error::io_failure(16, "write error", e));
                }
                if written == 0 {
                    return Err(io::Error::new(
                        ErrorKind::WriteZero,
                        "splice output made no progress",
                    ));
                }
                display.record(written as u64, written as u64, None);
                left -= written as usize;
            }
        } else {
            // A direct kernel call has already written these bytes. Account
            // before sleeping so live queries never trail successful output.
            display.record(n as u64, n as u64, None);
            throttle.before(n as u64, display)?;
        }
        display.tick(false)?;
    }
}

/// Allocation with the alignment required by direct-I/O filesystem operations.
/// Tail operations still use cached I/O when their length/offset isn't aligned.
pub struct AlignedBuffer {
    pointer: std::ptr::NonNull<u8>,
    layout: std::alloc::Layout,
}
impl AlignedBuffer {
    pub fn new(size: usize) -> io::Result<Self> {
        let size = size
            .checked_add(4095)
            .ok_or_else(|| io::Error::other("buffer overflow"))?
            / 4096
            * 4096;
        let layout = std::alloc::Layout::from_size_align(size, 4096).map_err(io::Error::other)?;
        // SAFETY: layout has nonzero size and valid power-of-two alignment.
        let pointer = std::ptr::NonNull::new(unsafe { std::alloc::alloc_zeroed(layout) })
            .ok_or_else(|| io::Error::other("cannot allocate transfer buffer"))?;
        Ok(Self { pointer, layout })
    }
    pub fn slice(&mut self) -> &mut [u8] {
        // SAFETY: allocation is live, initialized and uniquely borrowed by self.
        unsafe { std::slice::from_raw_parts_mut(self.pointer.as_ptr(), self.layout.size()) }
    }
}
impl Drop for AlignedBuffer {
    fn drop(&mut self) {
        // SAFETY: this allocation was obtained using exactly this layout.
        unsafe {
            std::alloc::dealloc(self.pointer.as_ptr(), self.layout);
        }
    }
}

#[cfg(target_os = "linux")]
pub struct DirectGuard {
    fd: RawFd,
    flags: i32,
}
#[cfg(target_os = "linux")]
impl DirectGuard {
    pub fn enable(fd: RawFd) -> io::Result<Self> {
        // SAFETY: fcntl reads/sets status flags on a live descriptor.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_DIRECT) } < 0 {
            return Err(io::Error::new(
                ErrorKind::Unsupported,
                format!("cannot enable direct I/O: {}", io::Error::last_os_error()),
            ));
        }
        Ok(Self { fd, flags })
    }
}
#[cfg(target_os = "linux")]
impl Drop for DirectGuard {
    fn drop(&mut self) {
        unsafe {
            libc::fcntl(self.fd, libc::F_SETFL, self.flags);
        }
    }
}

#[cfg(target_os = "linux")]
fn direct_tail<T>(
    fd: RawFd,
    pointer: *const u8,
    length: usize,
    operation: impl FnOnce() -> io::Result<T>,
) -> io::Result<T> {
    // Filesystems usually require sector-aligned lengths and offsets; 4096 is
    // conservative across supported block sizes. pv must preserve a short tail.
    let position = unsafe { libc::lseek(fd, 0, libc::SEEK_CUR) };
    if pointer as usize % 4096 == 0 && length % 4096 == 0 && position >= 0 && position % 4096 == 0 {
        return operation();
    }
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags & !libc::O_DIRECT) } < 0 {
        return Err(io::Error::last_os_error());
    }
    let result = operation();
    let restored = unsafe { libc::fcntl(fd, libc::F_SETFL, flags) };
    if restored < 0 {
        return Err(io::Error::last_os_error());
    }
    result
}

fn read_retry(reader: &mut impl Read, buffer: &mut [u8]) -> io::Result<usize> {
    loop {
        match reader.read(buffer) {
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            result => return result,
        }
    }
}

fn write_retry(writer: &mut impl Write, buffer: &[u8]) -> io::Result<usize> {
    loop {
        match writer.write(buffer) {
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Ok(0) if !buffer.is_empty() => {
                return Err(io::Error::new(
                    ErrorKind::WriteZero,
                    "writer made no progress",
                ))
            }
            result => return result,
        }
    }
}

fn flush_checked(writer: &mut impl Write) -> io::Result<()> {
    writer
        .flush()
        .map_err(|e| crate::error::io_failure(16, "write error (final flush)", e))
}

#[cfg(test)]
mod fault_tests {
    use super::*;
    struct InterruptedReader {
        interrupted: bool,
    }
    impl Read for InterruptedReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if !self.interrupted {
                self.interrupted = true;
                return Err(ErrorKind::Interrupted.into());
            }
            buffer[0] = b'x';
            Ok(1)
        }
    }
    struct FaultWriter {
        interrupted: bool,
        zero: bool,
        data: Vec<u8>,
    }
    impl Write for FaultWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            if !self.interrupted {
                self.interrupted = true;
                return Err(ErrorKind::Interrupted.into());
            }
            if self.zero {
                return Ok(0);
            }
            let n = buffer.len().min(2);
            self.data.extend_from_slice(&buffer[..n]);
            Ok(n)
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::other("injected final flush failure"))
        }
    }
    #[test]
    fn interrupted_reads_retry_without_consuming_or_fabricating_bytes() {
        let mut reader = InterruptedReader { interrupted: false };
        let mut data = [0; 4];
        assert_eq!(read_retry(&mut reader, &mut data).unwrap(), 1);
        assert_eq!(data, [b'x', 0, 0, 0]);
    }
    #[test]
    fn partial_writes_return_actual_count_and_retry_interruptions() {
        let mut writer = FaultWriter {
            interrupted: false,
            zero: false,
            data: Vec::new(),
        };
        assert_eq!(write_retry(&mut writer, b"abcde").unwrap(), 2);
        assert_eq!(writer.data, b"ab");
        assert_eq!(write_retry(&mut writer, b"cde").unwrap(), 2);
        assert_eq!(write_retry(&mut writer, b"e").unwrap(), 1);
        assert_eq!(writer.data, b"abcde");
        assert_eq!(
            crate::error::status(&flush_checked(&mut writer).unwrap_err()),
            16
        );
    }
    #[test]
    fn zero_writes_fail_instead_of_spinning() {
        let mut writer = FaultWriter {
            interrupted: true,
            zero: true,
            data: Vec::new(),
        };
        assert_eq!(
            write_retry(&mut writer, b"x").unwrap_err().kind(),
            ErrorKind::WriteZero
        );
    }
}
