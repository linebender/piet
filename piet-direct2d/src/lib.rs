//! The Direct2D backend for the Piet 2D graphics abstraction.

use std::borrow::Cow;

use winapi::shared::basetsd::UINT32;
use winapi::um::dcommon::D2D_SIZE_U;

use dxgi::Format;

use direct2d::brush::{Brush, GenericBrush, SolidColorBrush};
use direct2d::enums::{
    AlphaMode, BitmapInterpolationMode, DrawTextOptions, FigureBegin, FigureEnd, FillMode,
};
use direct2d::geometry::path::{FigureBuilder, GeometryBuilder};
use direct2d::geometry::Path;
use direct2d::image::Bitmap;
use direct2d::layer::Layer;
use direct2d::math::{
    BezierSegment, Matrix3x2F, Point2F, QuadBezierSegment, RectF, SizeU, Vector2F,
};
use direct2d::render_target::{GenericRenderTarget, RenderTarget};

use directwrite::text_format::TextFormatBuilder;
use directwrite::text_layout;
use directwrite::TextFormat;

use kurbo::{Affine, PathEl, Rect, Shape, Vec2};

use piet::{
    FillRule, Font, FontBuilder, ImageFormat, InterpolationMode, RenderContext, RoundFrom,
    RoundInto, TextLayout, TextLayoutBuilder,
};

pub struct D2DRenderContext<'a> {
    factory: &'a direct2d::Factory,
    dwrite: &'a directwrite::Factory,
    // This is an owned clone, but after some direct2d refactor, it's likely we'll
    // hold a mutable reference.
    rt: GenericRenderTarget,

    /// The context state stack. There is always at least one, until finishing.
    ctx_stack: Vec<CtxState>,
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

#[derive(Default)]
struct CtxState {
    transform: Affine,

    // Note: when we start pushing both layers and axis aligned clips, this will
    // need to keep track of which is which. But for now, keep it simple.
    n_layers_pop: usize,
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
            ctx_stack: vec![CtxState::default()],
        }
    }

    fn current_transform(&self) -> Affine {
        self.ctx_stack.last().unwrap().transform
    }

    fn pop_state(&mut self) {
        let old_state = self.ctx_stack.pop().unwrap();
        for _ in 0..old_state.n_layers_pop {
            self.rt.pop_layer();
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

/// Can't implement RoundFrom here because both types belong to other
/// crates. Consider moving to kurbo (with windows feature).
fn affine_to_matrix3x2f(affine: Affine) -> Matrix3x2F {
    let a = affine.as_coeffs();
    Matrix3x2F::new([
        [a[0] as f32, a[1] as f32],
        [a[2] as f32, a[3] as f32],
        [a[4] as f32, a[5] as f32],
    ])
}

// TODO: consider adding to kurbo.
fn rect_to_rectf(rect: Rect) -> RectF {
    (
        rect.x0 as f32,
        rect.y0 as f32,
        rect.x1 as f32,
        rect.y1 as f32,
    )
        .into()
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
    shape: impl Shape,
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
        if let Some(b) = builder.take() {
            // TODO: think about what to do on error
            let _ = b.finish_figure().close();
        }
    }
    path
}

impl<'a> RenderContext for D2DRenderContext<'a> {
    type Point = Point2;
    type Coord = f32;
    type Brush = GenericBrush;
    type StrokeStyle = direct2d::stroke_style::StrokeStyle;

    type Font = D2DFont;
    type FontBuilder = D2DFontBuilder<'a>;
    type TextLayout = D2DTextLayout;
    type TextLayoutBuilder = D2DTextLayoutBuilder<'a>;

    type Image = Bitmap;

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

    fn fill(&mut self, shape: impl Shape, brush: &Self::Brush, fill_rule: FillRule) {
        // TODO: various special-case shapes, for efficiency
        let path = path_from_shape(self.factory, true, shape, fill_rule);
        self.rt.fill_geometry(&path, brush);
    }

    fn stroke(
        &mut self,
        shape: impl Shape,
        brush: &Self::Brush,
        width: impl RoundInto<Self::Coord>,
        style: Option<&Self::StrokeStyle>,
    ) {
        // TODO: various special-case shapes, for efficiency
        let path = path_from_shape(self.factory, false, shape, FillRule::EvenOdd);
        self.rt
            .draw_geometry(&path, brush, width.round_into(), style);
    }

    fn clip(&mut self, shape: impl Shape, fill_rule: FillRule) {
        // TODO: set size based on bbox of shape.
        if let Ok(layer) = Layer::create(&mut self.rt, None) {
            let path = path_from_shape(self.factory, true, shape, fill_rule);
            // TODO: we get a use-after-free crash if we don't do this. Almost certainly
            // this will be fixed in direct2d 0.3, so remove workaround when upgrading.
            let _clone = path.clone();
            let transform = affine_to_matrix3x2f(self.current_transform());
            self.rt
                .push_layer(&layer)
                .with_mask(path)
                .with_mask_transform(transform)
                .push();
            self.ctx_stack.last_mut().unwrap().n_layers_pop += 1;
        } else {
            // TODO: error handling. Very unlikely to happen but maybe?
        }
    }

    fn new_font_by_name(
        &mut self,
        name: &str,
        size: impl RoundInto<Self::Coord>,
    ) -> Self::FontBuilder {
        // Note: the name is cloned here, rather than applied using `with_family` for
        // lifetime reasons. Maybe there's a better approach.
        D2DFontBuilder {
            builder: TextFormat::create(self.dwrite).with_size(size.round_into()),
            name: name.to_owned(),
        }
    }

    fn new_text_layout(&mut self, font: &Self::Font, text: &str) -> Self::TextLayoutBuilder {
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
        layout: &Self::TextLayout,
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

    fn save(&mut self) {
        let new_state = CtxState {
            transform: self.current_transform(),
            n_layers_pop: 0,
        };
        self.ctx_stack.push(new_state);
    }

    fn restore(&mut self) {
        if self.ctx_stack.len() <= 1 {
            panic!("restore without corresponding save");
        }
        self.pop_state();
        // Move this code into impl to avoid duplication with transform?
        self.rt
            .set_transform(&affine_to_matrix3x2f(self.current_transform()));
    }

    // TODO: should we panic on unbalanced stack? Maybe warn? Maybe have an
    // error result that reports the problem?
    //
    // Discussion question: should this subsume EndDraw, with BeginDraw on
    // D2DRenderContext creation? I'm thinking not, as the shell might want
    // to do other stuff, possibly related to incremental paint.
    fn finish(&mut self) {
        self.pop_state();
    }

    fn transform(&mut self, transform: Affine) {
        self.ctx_stack.last_mut().unwrap().transform *= transform;
        self.rt
            .set_transform(&affine_to_matrix3x2f(self.current_transform()));
    }

    fn make_image(
        &mut self,
        width: usize,
        height: usize,
        buf: &[u8],
        format: ImageFormat,
    ) -> Self::Image {
        // TODO: this method _really_ needs error checking, so much can go wrong...
        let alpha_mode = match format {
            ImageFormat::Rgb => AlphaMode::Ignore,
            ImageFormat::RgbaPremul | ImageFormat::RgbaSeparate => AlphaMode::Premultiplied,
            _ => panic!("Unexpected image format {:?}", format),
        };
        let buf = match format {
            ImageFormat::Rgb => {
                let mut new_buf = vec![255; width * height * 4];
                for i in 0..width * height {
                    new_buf[i * 4 + 0] = buf[i * 3 + 0];
                    new_buf[i * 4 + 1] = buf[i * 3 + 1];
                    new_buf[i * 4 + 2] = buf[i * 3 + 2];
                }
                Cow::from(new_buf)
            }
            ImageFormat::RgbaSeparate => {
                let mut new_buf = vec![255; width * height * 4];
                // TODO (performance): this would be soooo much faster with SIMD
                fn premul(x: u8, a: u8) -> u8 {
                    let y = (x as u16) * (a as u16);
                    ((y + (y >> 8) + 0x80) >> 8) as u8
                }
                for i in 0..width * height {
                    let a = buf[i * 4 + 3];
                    new_buf[i * 4 + 0] = premul(buf[i * 4 + 0], a);
                    new_buf[i * 4 + 1] = premul(buf[i * 4 + 1], a);
                    new_buf[i * 4 + 2] = premul(buf[i * 4 + 2], a);
                    new_buf[i * 4 + 3] = a;
                }
                Cow::from(new_buf)
            }
            ImageFormat::RgbaPremul => Cow::from(buf),
            _ => panic!("Unexpected image format {:?}", format),
        };
        Bitmap::create(&self.rt)
            .with_raw_data(
                SizeU(D2D_SIZE_U {
                    width: width as UINT32,
                    height: height as UINT32,
                }),
                &buf,
                width as UINT32 * 4,
            )
            .with_format(Format::R8G8B8A8Unorm)
            .with_alpha_mode(alpha_mode)
            .build()
            .expect("error creating bitmap")
    }

    fn draw_image(
        &mut self,
        image: &Self::Image,
        rect: impl Into<Rect>,
        interp: InterpolationMode,
    ) {
        let interp = match interp {
            InterpolationMode::NearestNeighbor => BitmapInterpolationMode::NearestNeighbor,
            InterpolationMode::Bilinear => BitmapInterpolationMode::Linear,
        };
        let src_size = image.get_size();
        let src_rect = (0.0, 0.0, src_size.0.width, src_size.0.height);
        self.rt
            .draw_bitmap(&image, rect_to_rectf(rect.into()), 1.0, interp, src_rect);
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
