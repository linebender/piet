//! Error conversion from D2D/DWrite to piet Error.

use std::fmt;

use directwrite::error::DWriteError;

use piet::Error;

/// The direct2d error type doesn't implement any error traits, so we newtype.
///
/// TODO: investigate getting either std::error::Error or maybe failure added
/// to direct2d.
#[derive(Debug)]
struct WrappedD2DError(direct2d::Error);

#[derive(Debug)]
struct WrappedDWriteError(DWriteError);

impl std::error::Error for WrappedD2DError {}
impl std::error::Error for WrappedDWriteError {}

impl fmt::Display for WrappedD2DError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Direct2D error: {}", self.0)
    }
}

impl fmt::Display for WrappedDWriteError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "DirectWrite error: {}", self.0)
    }
}

// Discussion question: is there a clean way to get this to automatically
// happen when the `?` macro is used on a D2DResult?
pub trait WrapError<T> {
    fn wrap(self) -> Result<T, Error>;
}

impl<T> WrapError<T> for Result<T, direct2d::Error> {
    fn wrap(self) -> Result<T, Error> {
        self.map_err(|e| {
            let e: Box<dyn std::error::Error> = Box::new(WrappedD2DError(e));
            e.into()
        })
    }
}

impl<T> WrapError<T> for Result<T, DWriteError> {
    fn wrap(self) -> Result<T, Error> {
        self.map_err(|e| {
            let e: Box<dyn std::error::Error> = Box::new(WrappedDWriteError(e));
            e.into()
        })
    }
}
