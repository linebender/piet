//! The common error type for piet operations.

use std::fmt;

/// An error that can occur while rendering 2D graphics.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    InvalidInput,
    NotSupported,
    StackUnbalance,
    BackendError(Box<dyn std::error::Error>),
    MissingFeature,
    MissingFont,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::InvalidInput => write!(f, "Invalid input"),
            Error::NotSupported => write!(f, "Option not supported"),
            Error::StackUnbalance => write!(f, "Stack unbalanced"),
            Error::MissingFont => write!(f, "A font could not be found"),
            Error::MissingFeature => write!(f, "A feature is not implemented on this backend"),
            Error::BackendError(e) => {
                write!(f, "Backend error: ")?;
                e.fmt(f)
            }
        }
    }
}

impl std::error::Error for Error {}

impl From<Box<dyn std::error::Error>> for Error {
    fn from(e: Box<dyn std::error::Error>) -> Error {
        Error::BackendError(e)
    }
}
