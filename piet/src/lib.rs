//! A 2D graphics abstraction.
//!
//! This crate contains types and interfaces that represent a traditional 2D
//! graphics API, in the tradition of [`PostScript`].
//!
//! This API can be implemented on various platforms, allowing drawing code
//! to be reused in an approximately consistent way. Various such implementations
//! exist, such as [`piet-cairo`], [`piet-coregraphics`], and [`piet-direct2d`].
//!
//! [`PostScript`]: https://en.wikipedia.org/wiki/PostScript
//! [`piet-cairo`]: https://crates.io/crates/piet-cairo
//! [`piet-coregraphics`]: https://crates.io/crates/piet-coregraphics
//! [`piet-direct2d`]: https://crates.io/crates/piet-direct2d

#![warn(missing_docs)]
#![deny(clippy::trivially_copy_pass_by_ref, broken_intra_doc_links)]

pub use kurbo;

/// utilities shared by various backends
pub mod util;

mod color;
mod conv;
mod error;
mod font;
mod gradient;
mod image;
mod null_renderer;
mod render_context;
mod shapes;
mod text;

#[cfg(feature = "samples")]
pub mod samples;

pub use crate::color::*;
pub use crate::conv::*;
pub use crate::error::*;
pub use crate::font::*;
pub use crate::gradient::*;
pub use crate::image::*;
pub use crate::null_renderer::*;
pub use crate::render_context::*;
pub use crate::shapes::*;
pub use crate::text::*;
