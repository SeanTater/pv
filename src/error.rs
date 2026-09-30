use std::error::Error;
use std::fmt;
use std::io;

/// Upstream's exit status bits distinguish invalid options, input failures and
/// output failures. Carry the category with the error rather than parsing text.
#[derive(Debug)]
pub struct Failure {
    pub code: i32,
    message: String,
}
impl Failure {
    pub fn new(code: i32, error: impl fmt::Display) -> Self {
        Self {
            code,
            message: error.to_string(),
        }
    }
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(f)
    }
}
impl Error for Failure {}

pub fn io_failure(code: i32, context: &str, error: io::Error) -> io::Error {
    io::Error::new(
        error.kind(),
        Failure::new(code, format!("{context}: {error}")),
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
