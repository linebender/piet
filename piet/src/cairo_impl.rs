//! Cairo implementation of 2D abstraction.

use cairo;

pub type Factory = ();

pub type Device = ();

pub struct ImageSurface {
    inner: cairo::ImageSurface,
}
