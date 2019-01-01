//! Fundamental graphics traits.

use kurbo::Vec2;

use crate::{RoundFrom, RoundInto};

pub trait RenderContext {
    /// Backends specify their own types for coordinates.
    type Point: Into<Vec2> + RoundFrom<Vec2>;
    type Coord: Into<f64> + RoundFrom<f64>;

    /// Clear the canvas with the given color.
    fn clear(&mut self, rgb: u32);

    fn line<V: RoundInto<Self::Point>, C: RoundInto<Self::Coord>>(
        &mut self,
        p0: V,
        p1: V,
        width: C,
    );
}
