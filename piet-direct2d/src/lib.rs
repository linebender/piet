//! The Direct2D backend for the Piet 2D graphics abstraction.

use std::borrow::Borrow;

use direct2d::brush::{Brush, GenericBrush, SolidColorBrush};
use direct2d::enums::{FigureBegin, FigureEnd, FillMode};
use direct2d::geometry::path::{FigureBuilder, GeometryBuilder};
use direct2d::geometry::Path;
use direct2d::math::{BezierSegment, Point2F, QuadBezierSegment};
use direct2d::render_target::{GenericRenderTarget, RenderTarget};

use kurbo::{PathEl, Vec2};

use piet::{FillRule, RenderContext, RoundFrom, RoundInto};

pub struct D2DRenderContext<'a> {
    factory: &'a direct2d::Factory,
    // This is an owned clone, but after some direct2d refactor, it's likely we'll
    // hold a mutable reference.
    rt: GenericRenderTarget,
}

impl<'a> D2DRenderContext<'a> {
    pub fn new<RT: RenderTarget>(
        factory: &'a direct2d::Factory,
        rt: &'a mut RT,
    ) -> D2DRenderContext<'a> {
        D2DRenderContext {
            factory,
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

impl RoundFrom<(f64, f64)> for Point2 {
    #[inline]
    fn round_from(vec: (f64, f64)) -> Point2 {
        Point2(Point2F::new(vec.0 as f32, vec.1 as f32))
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

enum PathBuilder<'a> {
    Geom(GeometryBuilder<'a>),
    Fig(FigureBuilder<'a>),
}

impl<'a> PathBuilder<'a> {
    fn finish_figure(self) -> GeometryBuilder<'a> {
        match self {
            PathBuilder::Geom(g) => g,
            PathBuilder::Fig(f) => f.end(),
        }
    }
}

fn to_point2f<P: RoundInto<Point2>>(p: P) -> Point2F {
    p.round_into().0
}

fn path_from_iterator(
    d2d: &direct2d::Factory,
    is_filled: bool,
    i: impl IntoIterator<Item = impl Borrow<PathEl>>,
    fill_rule: FillRule,
) -> Path {
    let mut path = Path::create(d2d).unwrap();
    {
        let mut g = path.open().unwrap();
        if fill_rule == FillRule::NonZero {
            g = g.fill_mode(FillMode::Winding);
        }
        let mut builder = Some(PathBuilder::Geom(g));
        for el in i.into_iter() {
            match *el.borrow() {
                PathEl::Moveto(p) => {
                    // TODO: we don't know this now. Will get fixed in direct2d crate.
                    let is_closed = is_filled;
                    if let Some(b) = builder.take() {
                        let g = b.finish_figure();
                        let begin = if is_filled {
                            FigureBegin::Filled
                        } else {
                            FigureBegin::Hollow
                        };
                        let end = if is_closed {
                            FigureEnd::Closed
                        } else {
                            FigureEnd::Open
                        };
                        let f = g.begin_figure(to_point2f(p), begin, end);
                        builder = Some(PathBuilder::Fig(f));
                    }
                }
                PathEl::Lineto(p) => {
                    if let Some(PathBuilder::Fig(f)) = builder.take() {
                        let f = f.add_line(to_point2f(p));
                        builder = Some(PathBuilder::Fig(f));
                    }
                }
                PathEl::Quadto(p1, p2) => {
                    if let Some(PathBuilder::Fig(f)) = builder.take() {
                        let q = QuadBezierSegment::new(to_point2f(p1), to_point2f(p2));
                        let f = f.add_quadratic_bezier(&q);
                        builder = Some(PathBuilder::Fig(f));
                    }
                }
                PathEl::Curveto(p1, p2, p3) => {
                    if let Some(PathBuilder::Fig(f)) = builder.take() {
                        let c = BezierSegment::new(to_point2f(p1), to_point2f(p2), to_point2f(p3));
                        let f = f.add_bezier(&c);
                        builder = Some(PathBuilder::Fig(f));
                    }
                }
                _ => (),
            }
        }
    }
    path
}

impl<'a> RenderContext for D2DRenderContext<'a> {
    type Point = Point2;
    type Coord = f32;
    type Brush = GenericBrush;
    type StrokeStyle = direct2d::stroke_style::StrokeStyle;

    fn clear(&mut self, rgb: u32) {
        self.rt.clear(rgb);
    }

    fn solid_brush(&mut self, rgba: u32) -> GenericBrush {
        SolidColorBrush::create(&self.rt)
            .with_color((rgba >> 8, ((rgba & 255) as f32) * (1.0 / 255.0)))
            .build()
            .unwrap()
            .to_generic() // This does an extra COM clone; avoid somehow?
    }

    fn line(
        &mut self,
        p0: impl RoundInto<Self::Point>,
        p1: impl RoundInto<Self::Point>,
        brush: &Self::Brush,
        width: impl RoundInto<Self::Coord>,
        style: Option<&Self::StrokeStyle>,
    ) {
        self.rt.draw_line(
            p0.round_into().0,
            p1.round_into().0,
            brush,
            width.round_into(),
            style,
        );
    }

    fn fill_path(
        &mut self,
        iter: impl IntoIterator<Item = impl Borrow<PathEl>>,
        brush: &Self::Brush,
        fill_rule: FillRule,
    ) {
        let path = path_from_iterator(self.factory, true, iter, fill_rule);
        self.rt.fill_geometry(&path, brush);
    }

    fn stroke_path(
        &mut self,
        iter: impl IntoIterator<Item = impl Borrow<PathEl>>,
        brush: &Self::Brush,
        width: impl RoundInto<Self::Coord>,
        style: Option<&Self::StrokeStyle>,
    ) {
        let path = path_from_iterator(self.factory, false, iter, FillRule::EvenOdd);
        self.rt
            .draw_geometry(&path, brush, width.round_into(), style);
    }
}
