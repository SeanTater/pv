use std::error::Error;
use std::fmt;
use std::io;

/// Upstream's exit status bits distinguish invalid options, input failures and
/// output failures. Carry the category with the error rather than parsing text.
#[derive(Debug)]
pub struct Failure {
    pub code: i32,
    message: String,
    cause: Option<io::Error>,
}
impl Failure {
    pub fn new(code: i32, error: impl fmt::Display) -> Self {
        Self {
            code,
            message: error.to_string(),
            cause: None,
        }
    }
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(f)
    }
}
impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause.as_ref().map(|error| error as &dyn Error)
    }
}

pub fn io_failure(code: i32, context: &str, error: io::Error) -> io::Error {
    io::Error::new(
        error.kind(),
        Failure {
            code,
            message: format!("{context}: {error}"),
            cause: Some(error),
        },
    )
}

pub fn status(error: &(dyn Error + 'static)) -> i32 {
    if let Some(failure) = error.downcast_ref::<Failure>() {
        return failure.code;
    }
    if let Some(error) = error.downcast_ref::<io::Error>() {
        if let Some(inner) = error.get_ref() {
            return status(inner);
        }
    }
    error.source().map(status).unwrap_or(1)
}

/// Error reporting always uses stderr, preserving the payload stream.
pub fn report(error: &(dyn Error + 'static), output: &mut impl io::Write) -> io::Result<()> {
    writeln!(output, "pv: {error}")?;
    let mut current = Some(error);
    let mut kind = None;
    let mut raw_code = None;
    while let Some(cause) = current {
        if let Some(io) = cause.downcast_ref::<io::Error>() {
            kind = Some(io.kind());
            raw_code = io.raw_os_error().or(raw_code);
            current = io.get_ref().map(|inner| inner as &dyn Error);
        } else {
            current = cause.source();
        }
    }
    let help = match kind {
        Some(io::ErrorKind::NotFound) => {
            Some("check that the input path and destination parent directory exist")
        }
        Some(io::ErrorKind::PermissionDenied) => {
            Some("check file permissions and directory access")
        }
        Some(io::ErrorKind::BrokenPipe) => {
            Some("the downstream reader stopped; check its exit status and errors")
        }
        Some(io::ErrorKind::WriteZero) => {
            Some("the destination stopped accepting data; check available space and device status")
        }
        Some(io::ErrorKind::Unsupported) => {
            Some("check platform support; try --no-splice for ordinary buffered copying")
        }
        _ if status(error) == 64 => Some("use --help for valid options, units and examples"),
        _ => None,
    };
    // ENOSPC is available on Unix even on the project's minimum Rust version.
    #[cfg(unix)]
    if raw_code == Some(libc::ENOSPC) {
        writeln!(output, "  help: free destination space or choose another output path; verify the copy before using it")?;
    }
    if let Some(help) = help {
        writeln!(output, "  help: {help}")?;
    }
    if debug_enabled() {
        writeln!(
            output,
            "  debug: pv {} on {}-{}; exit status {}; OS error {:?}",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH,
            status(error),
            raw_code
        )?;
        let mut current = Some(error);
        for depth in 0..16 {
            let Some(cause) = current else { break };
            writeln!(output, "  debug: cause[{depth}]: {cause}")?;
            current = if let Some(io) = cause.downcast_ref::<io::Error>() {
                io.get_ref().map(|inner| inner as &dyn Error)
            } else {
                cause.source()
            };
        }
    }
    Ok(())
}

pub fn debug_enabled() -> bool {
    std::env::var_os("PV_DEBUG").is_some_and(|value| value == "1")
}
