//! Direct2D implementation of 2D abstraction.

use direct2d;

// This type will probably expand to include direct2d and directwrite; but we'll
// tackle text later.
pub type Factory = direct2d::Factory;

pub type Device = direct2d::Device;

pub struct Context(direct2d::render_target::GenericRenderTarget);

pub struct ImageSurface {
}
