//! The Direct2D backend for the Piet 2D graphics abstraction.

use direct2d::brush::{Brush, GenericBrush, SolidColorBrush};
use direct2d::enums::{DrawTextOptions, FigureBegin, FigureEnd, FillMode};
use direct2d::geometry::path::{FigureBuilder, GeometryBuilder};
use direct2d::geometry::Path;
use direct2d::math::{BezierSegment, Point2F, QuadBezierSegment, Vector2F};
use direct2d::render_target::{GenericRenderTarget, RenderTarget};

use directwrite::text_format::TextFormatBuilder;
use directwrite::text_layout;
use directwrite::TextFormat;

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

pub struct D2DFontBuilder<'a> {
    builder: TextFormatBuilder<'a>,
    name: String,
}

pub struct D2DTextLayout(text_layout::TextLayout);

pub struct D2DTextLayoutBuilder<'a> {
    builder: text_layout::TextLayoutBuilder<'a>,
    format: TextFormat,
    text: String,
}

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

impl<'a> RenderContext for D2DRenderContext<'a> {
    type Point = Point2;
    type Coord = f32;
    type Brush = GenericBrush;
    type StrokeStyle = direct2d::stroke_style::StrokeStyle;

    type F = D2DFont;
    type FBuilder = D2DFontBuilder<'a>;
    type TL = D2DTextLayout;
    type TLBuilder = D2DTextLayoutBuilder<'a>;

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

    fn new_font_by_name(
        &mut self,
        name: &str,
        size: impl RoundInto<Self::Coord>,
    ) -> Self::FBuilder {
        // Note: the name is cloned here, rather than applied using `with_family` for
        // lifetime reasons. Maybe there's a better approach.
        D2DFontBuilder {
            builder: TextFormat::create(self.dwrite).with_size(size.round_into()),
            name: name.to_owned(),
        }
    }

    fn new_text_layout(&mut self, font: &Self::F, text: &str) -> Self::TLBuilder {
        // Same consideration as above, we clone the font and text for lifetime
        // reasons.
        D2DTextLayoutBuilder {
            builder: text_layout::TextLayout::create(self.dwrite),
            format: font.0.clone(),
            text: text.to_owned(),
        }
    }

    fn draw_text(
        &mut self,
        layout: &Self::TL,
        pos: impl RoundInto<Self::Point>,
        brush: &Self::Brush,
    ) {
        // TODO: set ENABLE_COLOR_FONT on Windows 8.1 and above, need version sniffing.
        let mut line_metrics = Vec::with_capacity(1);
        layout.0.get_line_metrics(&mut line_metrics);
        if line_metrics.is_empty() {
            // Layout is empty, don't bother drawing.
            return;
        }
        // Direct2D takes upper-left, so adjust for baseline.
        let pos = pos.round_into().0;
        let pos = pos - Vector2F::new(0.0, line_metrics[0].baseline());
        let text_options = DrawTextOptions::NONE;

        self.rt
            .draw_text_layout(pos, &layout.0, brush, text_options);
    }
}

impl<'a> FontBuilder for D2DFontBuilder<'a> {
    type Out = D2DFont;

    fn build(self) -> Self::Out {
        D2DFont(self.builder.with_family(&self.name).build().unwrap())
    }
}

impl Font for D2DFont {}

impl<'a> TextLayoutBuilder for D2DTextLayoutBuilder<'a> {
    type Out = D2DTextLayout;

    fn build(self) -> Self::Out {
        D2DTextLayout(
            self.builder
                .with_text(&self.text)
                .with_font(&self.format)
                .with_width(1e6) // TODO: probably want to support wrapping
                .with_height(1e6)
                .build()
                .unwrap(),
        )
    }
}

impl TextLayout for D2DTextLayout {
    type Coord = f32;

    fn width(&self) -> f32 {
        self.0.get_metrics().width()
    }
}
