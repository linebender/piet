//! The common error type for piet operations.

use std::fmt;

/// An error that can occur while rendering 2D graphics.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A function was passed an invalid input.
    InvalidInput,
    /// Something is impossible on the current platform.
    NotSupported,
    /// Something is possible, but not yet implemented.
    Unimplemented,
    /// Piet was compiled without a required feature.
    MissingFeature(&'static str),
    /// A stack pop failed.
    StackUnbalance,
    /// The backend failed unexpectedly.
    BackendError(Box<dyn std::error::Error>),
    /// A font could not be found.
    MissingFont,
    /// Font data could not be loaded.
    FontLoadingFailed,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::InvalidInput => write!(f, "Invalid input"),
            Error::NotSupported => write!(f, "Not supported on the current backend"),
            Error::StackUnbalance => write!(f, "Stack unbalanced"),
            Error::MissingFont => write!(f, "A font could not be found"),
            Error::FontLoadingFailed => write!(f, "A font could not be loaded"),
            Error::Unimplemented => write!(
                f,
                "This functionality is not yet implemented for this backend"
            ),
            Error::MissingFeature(feature) => write!(f, "Missing feature '{}'", feature),
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
