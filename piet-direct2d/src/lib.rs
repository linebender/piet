//! The Direct2D backend for the Piet 2D graphics abstraction.

use direct2d::brush::SolidColorBrush;
use direct2d::math::Point2F;
use direct2d::render_target::{GenericRenderTarget, RenderTarget};

use kurbo::Vec2;

use piet::{RenderContext, RoundFrom, RoundInto};

/// It is an interesting question, whether to wrap, or whether this should move into
/// piet proper under a feature. (We want to impl RenderContext for this, but we can't
/// impl RenderContext for GenericRenderTarget directly here due to coherence).
pub struct D2DRenderContext {
    rt: GenericRenderTarget,
}

impl D2DRenderContext {
    pub fn new<RT: RenderTarget>(rt: &RT) -> D2DRenderContext {
        D2DRenderContext {
            rt: rt.as_generic(),
        }
    }
}

/// This is wrapped for coherence reasons.
///
/// TODO: consider using Point2F instead, and moving conversions into kurbo.
pub struct Point2(Point2F);

impl From<Point2F> for Point2 {
    #[inline]
    fn from(vec: Point2F) -> Point2 {
        Point2(vec.into())
    }
}

impl From<(f32, f32)> for Point2 {
    #[inline]
    fn from(vec: (f32, f32)) -> Point2 {
        Point2(Point2F::new(vec.0, vec.1))
    }
}

// TODO: Maybe there's some blanket implementation that would cover this and
// not cause coherence problems.
impl RoundFrom<(f32, f32)> for Point2 {
    #[inline]
    fn round_from(vec: (f32, f32)) -> Point2 {
        Point2(Point2F::new(vec.0, vec.1))
    }
}

impl RoundFrom<Vec2> for Point2 {
    #[inline]
    fn round_from(vec: Vec2) -> Point2 {
        Point2(Point2F::new(vec.x as f32, vec.y as f32))
    }
}

impl From<Point2> for Vec2 {
    #[inline]
    fn from(vec: Point2) -> Vec2 {
        Vec2::new(vec.0.x as f64, vec.0.y as f64)
    }
}

impl RenderContext for D2DRenderContext {
    type Point = Point2;
    type Coord = f32;

    fn clear(&mut self, rgb_color: u32) {
        self.rt.clear(rgb_color);
    }

    fn line<V: RoundInto<Point2>, C: RoundInto<f32>>(&mut self, p0: V, p1: V, width: C) {
        let brush = SolidColorBrush::create(&self.rt)
            .with_color(0x00_00_80)
            .build()
            .unwrap();
        self.rt.draw_line(
            p0.round_into().0,
            p1.round_into().0,
            &brush,
            width.round_into(),
            None,
        );
    }
}
