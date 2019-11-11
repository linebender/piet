#![cfg(windows)]
//! The Direct2D backend for the Piet 2D graphics abstraction.

mod conv;
pub mod error;
mod text;

use crate::conv::{
    affine_to_matrix3x2f, color_to_colorf, convert_stroke_style, gradient_stop_to_d2d,
    rect_to_rectf, to_point2f,
};
use crate::error::WrapError;
pub use text::*;

use std::borrow::Cow;

use winapi::shared::basetsd::UINT32;
use winapi::um::dcommon::D2D_SIZE_U;

use dxgi::Format;

use direct2d::brush::gradient::linear::LinearGradientBrushBuilder;
use direct2d::brush::gradient::radial::RadialGradientBrushBuilder;
pub use direct2d::brush::GenericBrush;
use direct2d::brush::{Brush, SolidColorBrush};
use direct2d::enums::{
    AlphaMode, BitmapInterpolationMode, DrawTextOptions, FigureBegin, FigureEnd, FillMode,
};
use direct2d::geometry::path::{FigureBuilder, GeometryBuilder};
use direct2d::geometry::Path;
use direct2d::image::Bitmap;
use direct2d::layer::Layer;
use direct2d::math::{BezierSegment, QuadBezierSegment, SizeU, Vector2F};
use direct2d::render_target::{GenericRenderTarget, RenderTarget};

use piet::kurbo::{Affine, PathEl, Point, Rect, Shape};

use piet::{
    new_error, Color, Error, ErrorKind, FixedGradient,
    ImageFormat, InterpolationMode, IntoBrush, RenderContext,
    StrokeStyle,
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

// The setting of 1e-3 is extremely conservative (absolutely no
// differences should be visible) but setting a looser tolerance is
// likely a tiny performance improvement. We could fine-tune based on
// empirical study of both quality and performance.
const BEZ_TOLERANCE: f64 = 1e-3;

fn path_from_shape(
    d2d: &direct2d::Factory,
    is_filled: bool,
    shape: impl Shape,
    fill_mode: FillMode,
) -> Result<Path, Error> {
    let mut path = Path::create(d2d).wrap()?;
    {
        let mut g = path.open().wrap()?;
        if fill_mode == FillMode::Winding {
            g = g.fill_mode(fill_mode);
        }
        let mut builder = Some(PathBuilder::Geom(g));
        // Note: this is the allocate + clone version so we can scan forward
        // to determine whether the path is closed. Switch to non-allocating
        // version when updating the direct2d bindings.
        let bez_path = shape.into_bez_path(BEZ_TOLERANCE);
        let bez_elements = bez_path.elements();
        for i in 0..bez_elements.len() {
            let el = &bez_elements[i];
            match el {
                PathEl::MoveTo(p) => {
                    let mut is_closed = is_filled;
                    // Scan forward to see if we need to close the path. The
                    // need for this will go away when we udpate direct2d.
                    if !is_filled {
                        for close_el in &bez_elements[i + 1..] {
                            match close_el {
                                PathEl::MoveTo(_) => break,
                                PathEl::ClosePath => {
                                    is_closed = true;
                                    break;
                                }
                                _ => (),
                            }
                        }
                    }
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
                        let f = g.begin_figure(to_point2f(*p), begin, end);
                        builder = Some(PathBuilder::Fig(f));
                    }
                }
                PathEl::LineTo(p) => {
                    if let Some(PathBuilder::Fig(f)) = builder.take() {
                        let f = f.add_line(to_point2f(*p));
                        builder = Some(PathBuilder::Fig(f));
                    }
                }
                PathEl::QuadTo(p1, p2) => {
                    if let Some(PathBuilder::Fig(f)) = builder.take() {
                        let q = QuadBezierSegment::new(to_point2f(*p1), to_point2f(*p2));
                        let f = f.add_quadratic_bezier(&q);
                        builder = Some(PathBuilder::Fig(f));
                    }
                }
                PathEl::CurveTo(p1, p2, p3) => {
                    if let Some(PathBuilder::Fig(f)) = builder.take() {
                        let c =
                            BezierSegment::new(to_point2f(*p1), to_point2f(*p2), to_point2f(*p3));
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
    type Brush = GenericBrush;

    type Text = D2DText<'a>;

    type TextLayout = D2DTextLayout;

    type Image = Bitmap;

    fn status(&mut self) -> Result<(), Error> {
        std::mem::replace(&mut self.err, Ok(()))
    }

    fn clear(&mut self, color: Color) {
        self.rt.clear(color.as_rgba_u32() >> 8);
    }

    fn solid_brush(&mut self, color: Color) -> GenericBrush {
        SolidColorBrush::create(&self.rt)
            .with_color(color_to_colorf(color))
            .build()
            .wrap()
            .expect("error creating solid brush")
            .to_generic() // This does an extra COM clone; avoid somehow?
    }

    fn gradient(&mut self, gradient: impl Into<FixedGradient>) -> Result<GenericBrush, Error> {
        match gradient.into() {
            FixedGradient::Linear(linear) => {
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
            FixedGradient::Radial(radial) => {
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

    fn fill(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>) {
        // TODO: various special-case shapes, for efficiency
        let brush = brush.make_brush(self, || shape.bounding_box());
        match path_from_shape(self.factory, true, shape, FillMode::Winding) {
            Ok(path) => self.rt.fill_geometry(&path, &*brush),
            Err(e) => self.err = Err(e),
        }
    }

    fn fill_even_odd(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>) {
        // TODO: various special-case shapes, for efficiency
        let brush = brush.make_brush(self, || shape.bounding_box());
        match path_from_shape(self.factory, true, shape, FillMode::Alternate) {
            Ok(path) => self.rt.fill_geometry(&path, &*brush),
            Err(e) => self.err = Err(e),
        }
    }

    fn stroke(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>, width: f64) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        // TODO: various special-case shapes, for efficiency
        let path = match path_from_shape(self.factory, false, shape, FillMode::Alternate) {
            Ok(path) => path,
            Err(e) => {
                self.err = Err(e);
                return;
            }
        };
        let width = width as f32;
        self.rt.draw_geometry(&path, &*brush, width, None);
    }

    fn stroke_styled(
        &mut self,
        shape: impl Shape,
        brush: &impl IntoBrush<Self>,
        width: f64,
        style: &StrokeStyle,
    ) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        // TODO: various special-case shapes, for efficiency
        let path = match path_from_shape(self.factory, false, shape, FillMode::Alternate) {
            Ok(path) => path,
            Err(e) => {
                self.err = Err(e);
                return;
            }
        };
        let width = width as f32;
        let style = convert_stroke_style(self.factory, style, width)
            .expect("stroke style conversion failed");
        self.rt.draw_geometry(&path, &*brush, width, Some(&style));
    }

    fn clip(&mut self, shape: impl Shape) {
        // TODO: set size based on bbox of shape.
        let layer = match Layer::create(&mut self.rt, None).wrap() {
            Ok(layer) => layer,
            Err(e) => {
                self.err = Err(e);
                return;
            }
        };
        let path = match path_from_shape(self.factory, true, shape, FillMode::Winding) {
            Ok(path) => path,
            Err(e) => {
                self.err = Err(e);
                return;
            }
        };
        // TODO: we get a use-after-free crash if we don't do this. Almost certainly
        // this will be fixed in direct2d 0.3, so remove workaround when upgrading.
        let _clone = path.clone();
        self.rt.push_layer(&layer).with_mask(path).push();
        self.ctx_stack.last_mut().unwrap().n_layers_pop += 1;
    }

    fn text(&mut self) -> &mut Self::Text {
        &mut self.inner_text
    }

    fn draw_text(
        &mut self,
        layout: &Self::TextLayout,
        pos: impl Into<Point>,
        brush: &impl IntoBrush<Self>,
    ) {
        // TODO: bounding box for text
        let brush = brush.make_brush(self, || Rect::ZERO);
        let mut line_metrics = Vec::with_capacity(1);
        layout.layout.get_line_metrics(&mut line_metrics);
        if line_metrics.is_empty() {
            // Layout is empty, don't bother drawing.
            return;
        }
        // Direct2D takes upper-left, so adjust for baseline.
        let pos = to_point2f(pos.into());
        let pos = pos - Vector2F::new(0.0, line_metrics[0].baseline());
        // TODO: set ENABLE_COLOR_FONT on Windows 8.1 and above, need version sniffing.
        let text_options = DrawTextOptions::NONE;

        self.rt
            .draw_text_layout(pos, &layout.layout, &*brush, text_options);
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

impl<'a> IntoBrush<D2DRenderContext<'a>> for GenericBrush {
    fn make_brush<'b>(
        &'b self,
        _piet: &mut D2DRenderContext,
        _bbox: impl FnOnce() -> Rect,
    ) -> std::borrow::Cow<'b, GenericBrush> {
        Cow::Borrowed(self)
    }
}

