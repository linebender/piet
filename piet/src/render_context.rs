//! Fundamental graphics traits.

use kurbo::{Shape, Vec2};

use crate::{RoundFrom, RoundInto};

/// A fill rule for resolving winding numbers.
#[derive(Clone, Copy, PartialEq)]
pub enum FillRule {
    /// Fill everything with a non-zero winding number.
    NonZero,
    /// Fill everything with an odd winding number.
    EvenOdd,
}

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
    fn fill(
        &mut self,
        shape: &impl Shape,
        brush: &Self::Brush,
        fill_rule: FillRule,
    );
}
