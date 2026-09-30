//! Coordinate terminal rows between Rust pv processes without touching stdin.
use std::io;

#[cfg(unix)]
mod unix {
    use super::*;
    use std::fs::{File, OpenOptions};
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt},
    };
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct Lock<'a>(&'a File);
    impl<'a> Lock<'a> {
        fn acquire(file: &'a File) -> io::Result<Self> {
            loop {
                // SAFETY: flock operates on a live registry descriptor.
                if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } == 0 {
                    return Ok(Self(file));
                }
                let error = io::Error::last_os_error();
                if error.kind() != io::ErrorKind::Interrupted {
                    return Err(error);
                }
            }
        }
    }
    impl Drop for Lock<'_> {
        fn drop(&mut self) {
            unsafe {
                libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
            }
        }
    }

    pub struct Cursor {
        file: File,
        key: String,
        height: u16,
    }
    impl Cursor {
        pub fn new(height: Option<u16>) -> io::Result<Self> {
            let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
            // SAFETY: fstat initializes stat on success; fd 2 is the terminal.
            if unsafe { libc::fstat(2, stat.as_mut_ptr()) } < 0 {
                return Err(io::Error::last_os_error());
            }
            let stat = unsafe { stat.assume_init() };
            let path = crate::control::directory()?.join(format!("tty-{}.rows", stat.st_rdev));
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(path)?;
            let meta = file.metadata()?;
            if !meta.is_file()
                || meta.uid() != unsafe { libc::geteuid() }
                || meta.mode() & 0o077 != 0
            {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "cursor registry must be private",
                ));
            }
            let height = height.unwrap_or_else(|| {
                let mut size: libc::winsize = unsafe { std::mem::zeroed() };
                if unsafe { libc::ioctl(2, libc::TIOCGWINSZ, &mut size) } == 0 && size.ws_row > 0 {
                    size.ws_row
                } else {
                    25
                }
            });
            Ok(Self {
                file,
                key: format!(
                    "{}:{}",
                    std::process::id(),
                    SEQUENCE.fetch_add(1, Ordering::Relaxed)
                ),
                height,
            })
        }
        fn rows(&self) -> io::Result<Vec<String>> {
            let mut file = &self.file;
            file.seek(SeekFrom::Start(0))?;
            let mut text = String::new();
            file.take(16384).read_to_string(&mut text)?;
            Ok(text
                .lines()
                .filter(|key| {
                    key.split(':')
                        .next()
                        .and_then(|p| p.parse::<i32>().ok())
                        .is_some_and(|pid| pid > 0 && unsafe { libc::kill(pid, 0) } == 0)
                })
                .map(str::to_owned)
                .collect())
        }
        fn save(&self, rows: &[String]) -> io::Result<()> {
            let mut file = &self.file;
            file.seek(SeekFrom::Start(0))?;
            file.set_len(0)?;
            for key in rows {
                writeln!(file, "{key}")?;
            }
            file.flush()
        }
        pub fn draw(
            &self,
            stderr: &mut impl Write,
            line: &str,
            final_update: bool,
        ) -> io::Result<()> {
            let _lock = Lock::acquire(&self.file)?;
            let mut rows = self.rows()?;
            if !rows.contains(&self.key) {
                rows.push(self.key.clone());
                self.save(&rows)?;
            }
            let index = rows.iter().position(|key| key == &self.key).unwrap();
            let row = self
                .height
                .saturating_sub(rows.len().min(self.height as usize) as u16)
                + (index.min(self.height as usize - 1) + 1) as u16;
            // Save/restore are inside the same inter-process lock, so another
            // instance cannot overwrite the terminal's saved cursor meanwhile.
            write!(stderr, "\x1b7\x1b[{row};1H{line}\x1b[K\x1b8")?;
            if final_update {
                stderr.flush()?;
            }
            Ok(())
        }
    }
    impl Drop for Cursor {
        fn drop(&mut self) {
            if let Ok(_lock) = Lock::acquire(&self.file) {
                if let Ok(mut rows) = self.rows() {
                    rows.retain(|key| key != &self.key);
                    let _ = self.save(&rows);
                }
            }
        }
    }
}
#[cfg(unix)]
pub use unix::Cursor;
#[cfg(not(unix))]
pub struct Cursor;
#[cfg(not(unix))]
impl Cursor {
    pub fn new(_: Option<u16>) -> io::Result<Self> {
        Err(io::Error::other(
            "cursor coordination currently requires Unix",
        ))
    }
    pub fn draw(&self, _: &mut impl std::io::Write, _: &str, _: bool) -> io::Result<()> {
        Ok(())
    }
}

pub fn dimensions() -> Option<(u16, u16)> {
    #[cfg(unix)]
    {
        let mut size: libc::winsize = unsafe { std::mem::zeroed() };
        // SAFETY: ioctl writes into an initialized, live winsize structure.
        if unsafe { libc::ioctl(2, libc::TIOCGWINSZ, &mut size) } == 0
            && size.ws_col > 0
            && size.ws_row > 0
        {
            return Some((size.ws_col, size.ws_row));
        }
    }
    None
}
