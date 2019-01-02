//! Fundamental graphics traits.

use kurbo::{PathEl, Vec2};

use crate::{RoundFrom, RoundInto};

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

    fn line<V: RoundInto<Self::Point>, C: RoundInto<Self::Coord>>(
        &mut self,
        p0: V,
        p1: V,
        brush: &Self::Brush,
        width: C,
        style: Option<&Self::StrokeStyle>,
    );

    /// Fill a path given as an iterator.
    ///
    /// I'm also thinking of retained paths. But do we want to have a separate object for
    /// retained paths, or do we want to have a lightweight display list abstraction, so
    /// at worst you record a single `fill_path` into that?
    ///
    /// TODO: this is missing fill rule.
    fn fill_path<I: IntoIterator<Item = PathEl>>(&mut self, iter: I, brush: &Self::Brush);

    fn stroke_path<I: IntoIterator<Item = PathEl>, C: RoundInto<Self::Coord>>(
        &mut self,
        iter: I,
        brush: &Self::Brush,
        width: C,
        style: Option<&Self::StrokeStyle>,
    );
}
