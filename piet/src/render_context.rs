//! The main render context trait.

use kurbo::{Shape, Vec2};

use crate::{Font, FontBuilder, RoundFrom, RoundInto, TextLayout, TextLayoutBuilder};

/// A fill rule for resolving winding numbers.
#[derive(Clone, Copy, PartialEq)]
pub enum FillRule {
    /// Fill everything with a non-zero winding number.
    NonZero,
    /// Fill everything with an odd winding number.
    EvenOdd,
}

/// The main trait for rendering graphics.
///
/// This trait provides an API for drawing 2D graphics. In basic usage, it
/// wraps a surface of some kind, so that drawing commands paint onto the
/// surface. It can also be a recording context, creating a display list for
/// playback later.
///
/// The intent of the design is to be general so that any number of back-ends
/// can implement this trait.
///
/// Code that draws graphics will in general take `&mut impl RenderContext`.
pub trait RenderContext {
    /// The type of a 2D point, for this backend.
    ///
    /// Generally this needs to be a newtype so that the `RoundFrom` traits
    /// can be implemented on it. Possibly this can be relaxed in the future,
    /// as we move towards a standard `RoundFrom`.
    type Point: Into<Vec2> + RoundFrom<Vec2> + RoundFrom<(f32, f32)> + RoundFrom<(f64, f64)>;

    /// The type of 1D measurements, for example stroke width.
    ///
    /// Generally this will be either f32 or f64.
    type Coord: Into<f64> + RoundFrom<f64>;

    /// The type of a "brush".
    ///
    /// Initially just a solid RGBA color, but will probably expand to gradients.
    type Brush;

    /// Parameters for the style of stroke operations.
    type StrokeStyle;

    type FBuilder: FontBuilder<Out = Self::F>;
    type F: Font;

    type TLBuilder: TextLayoutBuilder<Out = Self::TL>;
    type TL: TextLayout;

    /// Create a new brush resource.
    ///
    /// TODO: figure out how to document lifetime and rebuilding requirements. Should
    /// that be the responsibility of the client, or should the back-end take
    /// responsiblity? We could have a cache that is flushed when the Direct2D
    /// render target is rebuilt. Solid brushes are super lightweight, but
    /// other potentially retained objects will be heavier.
    fn solid_brush(&mut self, rgba: u32) -> Self::Brush;

    /// Clear the canvas with the given color.
    fn clear(&mut self, rgb: u32);

    /// Stroke a shape.
    fn stroke(
        &mut self,
        shape: &impl Shape,
        brush: &Self::Brush,
        width: impl RoundInto<Self::Coord>,
        style: Option<&Self::StrokeStyle>,
    );

    /// Fill a shape.
    fn fill(&mut self, shape: &impl Shape, brush: &Self::Brush, fill_rule: FillRule);

    fn new_font_by_name(&mut self, name: &str) -> Self::FBuilder;

    fn new_text_layout(
        &mut self,
        size: impl RoundInto<Self::Coord>,
        text: &str,
    ) -> Self::TLBuilder;

    fn fill_text(
        &mut self,
        layout: &Self::TL,
        pos: impl RoundInto<Self::Point>,
        brush: &Self::Brush,
    );
}
