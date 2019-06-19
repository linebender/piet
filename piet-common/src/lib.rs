//! A piet backend appropriate for the current platform.
//!
//! This crate reexports the [piet crate][piet], alongside an appropriate backend
//! for the given platform. It also exposes [kurbo][], which defines shape and
//! curve types useful in drawing.
//!
//! The intention of this crate is to provide a single dependency that handles
//! the common piet use-case. If you have more complicated needs (such as
//! supporting multiple backends simultaneously) you should use crates such as
//! [piet][] and [piet-cairo][] directly.
//!
//! [piet]: https://crates.io/crates/piet
//! [kurbo]: https://crates.io/crates/kurbo
//! [piet-cairo]: https://crates.io/crates/piet-cairo

pub use piet::*;

#[doc(hidden)]
pub use piet::kurbo;

#[cfg(any(
    feature = "cairo",
    not(any(target_arch = "wasm32", target_os = "windows", feature = "direct2d"))
))]
#[path = "cairo_back.rs"]
mod backend;

#[cfg(any(feature = "d2d", all(target_os = "windows", not(feature = "cairo"))))]
#[path = "direct2d_back.rs"]
mod backend;

#[cfg(any(feature = "web", target_arch = "wasm32"))]
mod backend {
    pub use piet_web::*;
    pub type Piet<'a> = WebRenderContext<'a>;
}

#[doc(hidden)]
pub use backend::*;
