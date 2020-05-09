//! A 2D graphics abstraction.

pub use kurbo;

/// utilities shared by various backends
pub mod util;

mod color;
mod conv;
mod error;
mod gradient;
mod null_renderer;
mod render_context;
mod shapes;
mod text;

#[cfg(feature = "samples")]
pub mod samples;

pub use crate::color::*;
pub use crate::conv::*;
pub use crate::error::*;
pub use crate::gradient::*;
pub use crate::null_renderer::*;
pub use crate::render_context::*;
pub use crate::shapes::*;
pub use crate::text::*;

#[cfg(feature = "samples")]
pub use samples::draw_test_picture;
