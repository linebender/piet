//! The Direct2D backend for the Piet 2D graphics abstraction.

mod conv;
pub mod error;

use crate::conv::{
    affine_to_matrix3x2f, color_to_colorf, convert_stroke_style, gradient_stop_to_d2d,
    rect_to_rectf, to_point2f, Point2,
};
use crate::error::WrapError;

use std::borrow::Cow;

use winapi::shared::basetsd::UINT32;
use winapi::um::dcommon::D2D_SIZE_U;

use dxgi::Format;

use direct2d::brush::gradient::linear::LinearGradientBrushBuilder;
use direct2d::brush::gradient::radial::RadialGradientBrushBuilder;
use direct2d::brush::{Brush, GenericBrush, SolidColorBrush};
use direct2d::enums::{
    AlphaMode, BitmapInterpolationMode, DrawTextOptions, FigureBegin, FigureEnd, FillMode,
};
use direct2d::geometry::path::{FigureBuilder, GeometryBuilder};
use direct2d::geometry::Path;
use direct2d::image::Bitmap;
use direct2d::layer::Layer;
use direct2d::math::{BezierSegment, QuadBezierSegment, SizeU, Vector2F};
use direct2d::render_target::{GenericRenderTarget, RenderTarget};

use directwrite::text_format::TextFormatBuilder;
use directwrite::text_layout;
use directwrite::TextFormat;

use piet::kurbo::{Affine, PathEl, Rect, Shape};

use piet::{
    new_error, Color, Error, ErrorKind, FillRule, Font, FontBuilder, Gradient, ImageFormat,
    InterpolationMode, RenderContext, RoundInto, StrokeStyle, Text, TextLayout, TextLayoutBuilder,
};

pub struct D2DRenderContext<'a> {
    factory: &'a direct2d::Factory,
    inner_text: D2DText<'a>,
    // This is an owned clone, but after some direct2d refactor, it's likely we'll
    // hold a mutable reference.
    rt: GenericRenderTarget,

    /// The context state stack. There is always at least one, until finishing.
    ctx_stack: Vec<CtxState>,

    err: Result<(), Error>,
}

pub struct D2DText<'a> {
    dwrite: &'a directwrite::Factory,
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

impl<'b, 'a: 'b> D2DRenderContext<'a> {
    /// Create a new Piet RenderContext for the Direct2D RenderTarget.
    ///
    /// Note: the signature of this function has more restrictive lifetimes than
    /// the implementation requires, because we actually clone the RT, but this
    /// will likely change.
    pub fn new<RT: RenderTarget>(
        factory: &'a direct2d::Factory,
        dwrite: &'a directwrite::Factory,
        rt: &'b mut RT,
    ) -> D2DRenderContext<'b> {
        let inner_text = D2DText { dwrite };
        D2DRenderContext {
            factory,
            inner_text: inner_text,
            rt: rt.as_generic(),
            ctx_stack: vec![CtxState::default()],
            err: Ok(()),
        }
    }

    fn current_transform(&self) -> Affine {
        // This is an unwrap because we protect the invariant.
        self.ctx_stack.last().unwrap().transform
    }

    fn pop_state(&mut self) {
        // This is an unwrap because we protect the invariant.
        let old_state = self.ctx_stack.pop().unwrap();
        for _ in 0..old_state.n_layers_pop {
            self.rt.pop_layer();
        }
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

fn path_from_shape(
    d2d: &direct2d::Factory,
    is_filled: bool,
    shape: impl Shape,
    fill_rule: FillRule,
) -> Result<Path, Error> {
    let mut path = Path::create(d2d).wrap()?;
    {
        let mut g = path.open().wrap()?;
        if fill_rule == FillRule::NonZero {
            g = g.fill_mode(FillMode::Winding);
        }
        let mut builder = Some(PathBuilder::Geom(g));
        for el in shape.to_bez_path(1e-3) {
            match el {
                PathEl::MoveTo(p) => {
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
                PathEl::LineTo(p) => {
                    if let Some(PathBuilder::Fig(f)) = builder.take() {
                        let f = f.add_line(to_point2f(p));
                        builder = Some(PathBuilder::Fig(f));
                    }
                }
                PathEl::QuadTo(p1, p2) => {
                    if let Some(PathBuilder::Fig(f)) = builder.take() {
                        let q = QuadBezierSegment::new(to_point2f(p1), to_point2f(p2));
                        let f = f.add_quadratic_bezier(&q);
                        builder = Some(PathBuilder::Fig(f));
                    }
                }
                PathEl::CurveTo(p1, p2, p3) => {
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
    Ok(path)
}

impl<'a> RenderContext for D2DRenderContext<'a> {
    type Point = Point2;
    type Coord = f32;
    type Brush = GenericBrush;

    type Text = D2DText<'a>;

    type TextLayout = D2DTextLayout;

    type Image = Bitmap;

    fn status(&mut self) -> Result<(), Error> {
        std::mem::replace(&mut self.err, Ok(()))
    }

    fn clear(&mut self, color: Color) {
        self.rt.clear(color.as_rgba32() >> 8);
    }

    fn solid_brush(&mut self, color: Color) -> GenericBrush {
        SolidColorBrush::create(&self.rt)
            .with_color(color_to_colorf(color))
            .build()
            .wrap()
            .expect("error creating solid brush")
            .to_generic() // This does an extra COM clone; avoid somehow?
    }

    fn gradient(&mut self, gradient: Gradient) -> Result<GenericBrush, Error> {
        match gradient {
            Gradient::Linear(linear) => {
                let mut builder = LinearGradientBrushBuilder::new(&self.rt)
                    .with_start(to_point2f(linear.start))
                    .with_end(to_point2f(linear.end));
                for stop in &linear.stops {
                    builder = builder.with_stop(gradient_stop_to_d2d(stop));
                }
                let brush = builder.build().wrap()?;
                // Same concern about extra COM clone as above.
                Ok(brush.to_generic())
            }
            Gradient::Radial(radial) => {
                let radius = radial.radius as f32;
                let mut builder = RadialGradientBrushBuilder::new(&self.rt)
                    .with_center(to_point2f(radial.center))
                    .with_origin_offset(to_point2f(radial.origin_offset))
                    .with_radius(radius, radius);
                for stop in &radial.stops {
                    builder = builder.with_stop(gradient_stop_to_d2d(stop));
                }
                let brush = builder.build().wrap()?;
                // Ditto
                Ok(brush.to_generic())
            }
        }
    }

    fn fill(&mut self, shape: impl Shape, brush: &Self::Brush, fill_rule: FillRule) {
        // TODO: various special-case shapes, for efficiency
        match path_from_shape(self.factory, true, shape, fill_rule) {
            Ok(path) => self.rt.fill_geometry(&path, brush),
            Err(e) => self.err = Err(e),
        }
    }

    fn stroke(
        &mut self,
        shape: impl Shape,
        brush: &Self::Brush,
        width: impl RoundInto<Self::Coord>,
        style: Option<&StrokeStyle>,
    ) {
        // TODO: various special-case shapes, for efficiency
        let path = match path_from_shape(self.factory, false, shape, FillRule::EvenOdd) {
            Ok(path) => path,
            Err(e) => {
                self.err = Err(e);
                return;
            }
        };
        let width = width.round_into();
        let style = if let Some(style) = style {
            Some(convert_stroke_style(self.factory, style, width).expect("TODO"))
        } else {
            None
        };
        self.rt.draw_geometry(&path, brush, width, style.as_ref());
    }

    fn clip(&mut self, shape: impl Shape, fill_rule: FillRule) {
        // TODO: set size based on bbox of shape.
        let layer = match Layer::create(&mut self.rt, None).wrap() {
            Ok(layer) => layer,
            Err(e) => {
                self.err = Err(e);
                return;
            }
        };
        let path = match path_from_shape(self.factory, false, shape, fill_rule) {
            Ok(path) => path,
            Err(e) => {
                self.err = Err(e);
                return;
            }
        };
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
    }

    fn text(&mut self) -> &mut Self::Text {
        &mut self.inner_text
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

    fn save(&mut self) -> Result<(), Error> {
        let new_state = CtxState {
            transform: self.current_transform(),
            n_layers_pop: 0,
        };
        self.ctx_stack.push(new_state);
        Ok(())
    }

    fn restore(&mut self) -> Result<(), Error> {
        if self.ctx_stack.len() <= 1 {
            return Err(new_error(ErrorKind::StackUnbalance));
        }
        self.pop_state();
        // Move this code into impl to avoid duplication with transform?
        self.rt
            .set_transform(&affine_to_matrix3x2f(self.current_transform()));
        Ok(())
    }

    // Discussion question: should this subsume EndDraw, with BeginDraw on
    // D2DRenderContext creation? I'm thinking not, as the shell might want
    // to do other stuff, possibly related to incremental paint.
    fn finish(&mut self) -> Result<(), Error> {
        if self.ctx_stack.len() != 1 {
            return Err(new_error(ErrorKind::StackUnbalance));
        }
        self.pop_state();
        std::mem::replace(&mut self.err, Ok(()))
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
    ) -> Result<Self::Image, Error> {
        // TODO: this method _really_ needs error checking, so much can go wrong...
        let alpha_mode = match format {
            ImageFormat::Rgb => AlphaMode::Ignore,
            ImageFormat::RgbaPremul | ImageFormat::RgbaSeparate => AlphaMode::Premultiplied,
            _ => return Err(new_error(ErrorKind::NotSupported)),
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
            // This should be unreachable, we caught it above.
            _ => return Err(new_error(ErrorKind::NotSupported)),
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
            .wrap()
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

impl<'a> Text for D2DText<'a> {
    type Coord = f32;
    type FontBuilder = D2DFontBuilder<'a>;
    type Font = D2DFont;
    type TextLayoutBuilder = D2DTextLayoutBuilder<'a>;
    type TextLayout = D2DTextLayout;

    fn new_font_by_name(
        &mut self,
        name: &str,
        size: impl RoundInto<Self::Coord>,
    ) -> Result<Self::FontBuilder, Error> {
        // Note: the name is cloned here, rather than applied using `with_family` for
        // lifetime reasons. Maybe there's a better approach.
        Ok(D2DFontBuilder {
            builder: TextFormat::create(self.dwrite).with_size(size.round_into()),
            name: name.to_owned(),
        })
    }

    fn new_text_layout(
        &mut self,
        font: &Self::Font,
        text: &str,
    ) -> Result<Self::TextLayoutBuilder, Error> {
        // Same consideration as above, we clone the font and text for lifetime
        // reasons.
        Ok(D2DTextLayoutBuilder {
            builder: text_layout::TextLayout::create(self.dwrite),
            format: font.0.clone(),
            text: text.to_owned(),
        })
    }
}

impl<'a> FontBuilder for D2DFontBuilder<'a> {
    type Out = D2DFont;

    fn build(self) -> Result<Self::Out, Error> {
        Ok(D2DFont(
            self.builder.with_family(&self.name).build().wrap()?,
        ))
    }
}

impl Font for D2DFont {}

impl<'a> TextLayoutBuilder for D2DTextLayoutBuilder<'a> {
    type Out = D2DTextLayout;

    fn build(self) -> Result<Self::Out, Error> {
        Ok(D2DTextLayout(
            self.builder
                .with_text(&self.text)
                .with_font(&self.format)
                .with_width(1e6) // TODO: probably want to support wrapping
                .with_height(1e6)
                .build()
                .wrap()?,
        ))
    }
}

impl TextLayout for D2DTextLayout {
    type Coord = f32;

    fn width(&self) -> f32 {
        self.0.get_metrics().width()
    }
}
