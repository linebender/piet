//! Selection of a common back-end for piet.

pub type Point = <Piet<'static> as piet::RenderContext>::Point;
pub type Coord = <Piet<'static> as piet::RenderContext>::Coord;
pub type Brush = <Piet<'static> as piet::RenderContext>::Brush;
pub type Text<'a> = <Piet<'a> as piet::RenderContext>::Text;
pub type TextLayout = <Piet<'static> as piet::RenderContext>::TextLayout;
pub type Image = <Piet<'static> as piet::RenderContext>::Image;

pub type FontBuilder<'a> = <Text<'a> as piet::Text>::FontBuilder;
pub type Font = <Text<'static> as piet::Text>::Font;
pub type TextLayoutBuilder<'a> = <Text<'a> as piet::Text>::TextLayoutBuilder;

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
