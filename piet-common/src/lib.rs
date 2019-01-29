//! Selection of a common back-end for piet.

#[cfg(any(
    feature = "cairo",
    not(any(target_arch = "wasm32", target_os = "windows", feature = "direct2d"))
))]
mod cairo_back;

#[cfg(any(
    feature = "cairo",
    not(any(target_arch = "wasm32", target_os = "windows", feature = "direct2d"))
))]
pub use crate::cairo_back::*;

#[cfg(any(feature = "d2d", all(target_os = "windows", not(feature = "cairo"))))]
mod direct2d_back;

#[cfg(any(feature = "d2d", all(target_os = "windows", not(feature = "cairo"))))]
pub use crate::direct2d_back::*;

#[cfg(any(feature = "web", target_arch = "wasm32"))]
mod back {
    pub use piet_web::*;

    pub type Piet<'a> = WebRenderContext<'a>;
}

#[cfg(any(feature = "web", target_arch = "wasm32"))]
pub use crate::back::*;
