//! Support for piet Web back-end.

#[doc(hidden)]
pub use piet_web::*;
pub type Piet<'a> = WebRenderContext<'a>;

/// The associated brush type for this backend.
///
/// This type matches `RenderContext::Brush`
pub type Brush = piet_web::Brush;

/// The associated text factory for this backend.
///
/// This type matches `RenderContext::Text`
pub type PietText<'a> = WebRenderContext<'a>;

/// The associated font type for this backend.
///
/// This type matches `RenderContext::Text::Font`
pub type PietFont = WebFont;

/// The associated font builder for this backend.
///
/// This type matches `RenderContext::Text::FontBuilder`
pub type PietFontBuilder<'a> = WebFontBuilder;

/// The associated text layout type for this backend.
///
/// This type matches `RenderContext::Text::TextLayout`
pub type PietTextLayout = WebTextLayout;

/// The associated text layout builder for this backend.
///
/// This type matches `RenderContext::Text::TextLayoutBuilder`
pub type PietTextLayoutBuilder<'a> = WebTextLayoutBuilder;

/// The associated image type for this backend.
///
/// This type matches `RenderContext::Image`
pub type Image = WebImage;
