//! The common error type for piet operations.

use std::fmt;

/// An error that can occur while rendering 2D graphics.
#[derive(Debug)]
pub struct Error(Box<ErrorKind>);

#[derive(Debug)]
pub enum ErrorKind {
    InvalidInput,
    NotSupported,
    StackUnbalance,
    BackendError(Box<dyn std::error::Error>),
    #[doc(hidden)]
    _NonExhaustive,
    MissingFeature,
}

/// Create a new error of the given kind.
pub fn new_error(kind: ErrorKind) -> Error {
    Error(Box::new(kind))
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self.0 {
            ErrorKind::InvalidInput => write!(f, "Invalid input"),
            ErrorKind::NotSupported => write!(f, "Option not supported"),
            ErrorKind::StackUnbalance => write!(f, "Stack unbalanced"),
            ErrorKind::BackendError(ref e) => {
                write!(f, "Backend error: ")?;
                e.fmt(f)
            }
            _ => write!(f, "Unknown piet error (case not covered)"),
        }
    }
}

impl std::error::Error for Error {}

impl From<Box<dyn std::error::Error>> for Error {
    fn from(e: Box<dyn std::error::Error>) -> Error {
        new_error(ErrorKind::BackendError(e))
    }
}
