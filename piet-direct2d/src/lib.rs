//! The Direct2D backend for the Piet 2D graphics abstraction.

use direct2d::brush::{Brush, GenericBrush, SolidColorBrush};
use direct2d::enums::{FigureBegin, FigureEnd, FillMode};
use direct2d::geometry::path::{FigureBuilder, GeometryBuilder};
use direct2d::geometry::Path;
use direct2d::math::{BezierSegment, Point2F, QuadBezierSegment};
use direct2d::render_target::{GenericRenderTarget, RenderTarget};

use directwrite::TextFormat;
use directwrite::text_format::TextFormatBuilder;

use kurbo::{PathEl, Shape, Vec2};

use piet::{
    FillRule, Font, FontBuilder, RenderContext, RoundFrom, RoundInto, TextLayout, TextLayoutBuilder,
};

pub struct D2DRenderContext<'a> {
    factory: &'a direct2d::Factory,
    dwrite: &'a directwrite::Factory,
    // This is an owned clone, but after some direct2d refactor, it's likely we'll
    // hold a mutable reference.
    rt: GenericRenderTarget,
}

pub struct D2DFont(TextFormat);

pub struct D2DFontBuilder {
    dwrite: directwrite::Factory,
    name: String,
}

pub struct D2DTextLayout {}

pub struct D2DTextLayoutBuilder {}

impl<'a> D2DRenderContext<'a> {
    pub fn new<RT: RenderTarget>(
        factory: &'a direct2d::Factory,
        dwrite: &'a directwrite::Factory,
        rt: &'a mut RT,
    ) -> D2DRenderContext<'a> {
        D2DRenderContext {
            factory,
            dwrite,
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

fn path_from_shape(
    d2d: &direct2d::Factory,
    is_filled: bool,
    shape: &impl Shape,
    fill_rule: FillRule,
) -> Path {
    let mut path = Path::create(d2d).unwrap();
    {
        let mut g = path.open().unwrap();
        if fill_rule == FillRule::NonZero {
            g = g.fill_mode(FillMode::Winding);
        }
        let mut builder = Some(PathBuilder::Geom(g));
        for el in shape.to_bez_path(1e-3) {
            match el {
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

fn clone_dwrite(dwrite: &directwrite::Factory) -> directwrite::Factory {
    // Cloning the dwrite factory is a very hackish way to get around the lack
    // of GAT so that the D2DFontBuilder could hold a lifetime reference.
    //
    // TODO: reconsider life decisions.
    unsafe {
        let ptr = dwrite.get_raw();
        (*ptr).AddRef();
        directwrite::Factory::from_raw(ptr)
    }
}

impl<'a> RenderContext for D2DRenderContext<'a> {
    type Point = Point2;
    type Coord = f32;
    type Brush = GenericBrush;
    type StrokeStyle = direct2d::stroke_style::StrokeStyle;

    type F = D2DFont;
    type FBuilder = D2DFontBuilder;
    type TL = D2DTextLayout;
    type TLBuilder = D2DTextLayoutBuilder;

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

    fn fill(&mut self, shape: &impl Shape, brush: &Self::Brush, fill_rule: FillRule) {
        // TODO: various special-case shapes, for efficiency
        let path = path_from_shape(self.factory, true, shape, fill_rule);
        self.rt.fill_geometry(&path, brush);
    }

    fn stroke(
        &mut self,
        shape: &impl Shape,
        brush: &Self::Brush,
        width: impl RoundInto<Self::Coord>,
        style: Option<&Self::StrokeStyle>,
    ) {
        // TODO: various special-case shapes, for efficiency
        let path = path_from_shape(self.factory, false, shape, FillRule::EvenOdd);
        self.rt
            .draw_geometry(&path, brush, width.round_into(), style);
    }

    fn new_font_by_name(&mut self, name: &str) -> Self::FBuilder {
        D2DFontBuilder {
            dwrite: clone_dwrite(&self.dwrite),
            name: name.to_owned(),
        }
    }

    fn new_text_layout(
        &mut self,
        size: impl RoundInto<Self::Coord>,
        text: &str,
    ) -> Self::TLBuilder {
        D2DTextLayoutBuilder {}
    }

    fn fill_text(
        &mut self,
        layout: &Self::TL,
        pos: impl RoundInto<Self::Point>,
        brush: &Self::Brush,
    ) {}
}

impl FontBuilder for D2DFontBuilder {
    type Out = D2DFont;

    fn build(self) -> Self::Out {
        D2DFont(unimplemented!())
    }
}

impl Font for D2DFont {}

impl TextLayoutBuilder for D2DTextLayoutBuilder {
    type Out = D2DTextLayout;

    fn build(self) -> Self::Out {
        D2DTextLayout {}
    }
}

impl TextLayout for D2DTextLayout {
}
