#![cfg(windows)]
// allows for nice formatting for e.g. new_buf[i * 4 + 0] = premul(buf[i * 4 + 0, a)
#![allow(clippy::identity_op)]
#![deny(clippy::trivially_copy_pass_by_ref)]

//! The Direct2D backend for the Piet 2D graphics abstraction.

mod conv;
pub mod d2d;
pub mod d3d;
pub mod dwrite;
mod text;

use std::borrow::Cow;
use std::ops::Deref;

use associative_cache::{AssociativeCache, Capacity1024, HashFourWay, RoundRobinReplacement};

use winapi::um::d2d1::{
    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
    D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES, D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES,
};
use winapi::um::d2d1_1::{D2D1_COMPOSITE_MODE_SOURCE_OVER, D2D1_INTERPOLATION_MODE_LINEAR};
use winapi::um::dcommon::{D2D1_ALPHA_MODE_IGNORE, D2D1_ALPHA_MODE_PREMULTIPLIED};

use piet::kurbo::{Affine, PathEl, Point, Rect, Shape, Size};

use piet::{
    Color, Error, FixedGradient, Image, ImageFormat, InterpolationMode, IntoBrush, RenderContext,
    StrokeStyle,
};

use crate::d2d::{wrap_unit, Layer};
pub use crate::d2d::{D2DDevice, D2DFactory, DeviceContext as D2DDeviceContext};
pub use crate::dwrite::DwriteFactory;
pub use crate::text::{D2DText, D2DTextLayout, D2DTextLayoutBuilder};

use crate::conv::{
    affine_to_matrix3x2f, color_to_colorf, convert_stroke_style, gradient_stop_to_d2d,
    rect_to_rectf, to_point2f,
};
use crate::d2d::{Bitmap, Brush, DeviceContext, FillRule, Geometry};

pub struct D2DRenderContext<'a> {
    factory: &'a D2DFactory,
    inner_text: D2DText,
    rt: &'a mut D2DDeviceContext,

    /// The context state stack. There is always at least one, until finishing.
    ctx_stack: Vec<CtxState>,

    layers: Vec<(Geometry, Layer)>,

    err: Result<(), Error>,

    brush_cache: AssociativeCache<u32, Brush, Capacity1024, HashFourWay, RoundRobinReplacement>,
}

#[derive(Default)]
struct CtxState {
    transform: Affine,

    // Note: when we start pushing both layers and axis aligned clips, this will
    // need to keep track of which is which. But for now, keep it simple.
    n_layers_pop: usize,
}

impl<'b, 'a: 'b> D2DRenderContext<'a> {
    /// Create a new Piet RenderContext for the Direct2D DeviceContext.
    ///
    /// TODO: check signature.
    pub fn new(
        factory: &'a D2DFactory,
        dwrite: DwriteFactory,
        rt: &'b mut DeviceContext,
    ) -> D2DRenderContext<'b> {
        let inner_text = D2DText::new(dwrite);
        D2DRenderContext {
            factory,
            inner_text,
            rt,
            layers: vec![],
            ctx_stack: vec![CtxState::default()],
            err: Ok(()),
            brush_cache: Default::default(),
        }
    }

    fn pop_state(&mut self) {
        // This is an unwrap because we protect the invariant.
        let old_state = self.ctx_stack.pop().unwrap();
        for _ in 0..old_state.n_layers_pop {
            self.rt.pop_layer();
            self.layers.pop();
        }
    }

    /// Check whether drawing operations have finished.
    ///
    /// Clients should call this before extracting or presenting the contents of
    /// the drawing surface.
    pub fn assert_finished(&mut self) {
        assert!(
            self.ctx_stack.last().unwrap().n_layers_pop == 0,
            "Need to call finish() before using the contents"
        );
    }
}

// The setting of 1e-3 is extremely conservative (absolutely no
// differences should be visible) but setting a looser tolerance is
// likely a tiny performance improvement. We could fine-tune based on
// empirical study of both quality and performance.
const BEZ_TOLERANCE: f64 = 1e-3;

fn geometry_from_shape(
    d2d: &D2DFactory,
    is_filled: bool,
    shape: impl Shape,
    fill_rule: FillRule,
) -> Result<Geometry, Error> {
    // TODO: Do something special for line?
    if let Some(rect) = shape.as_rect() {
        Ok(d2d.create_rect_geometry(rect)?.into())
    } else if let Some(round_rect) = shape
        .as_rounded_rect()
        .filter(|r| r.radii().as_single_radius().is_some())
    {
        Ok(d2d
            .create_round_rect_geometry(
                round_rect.rect(),
                round_rect.radii().as_single_radius().unwrap(),
            )?
            .into())
    } else if let Some(circle) = shape.as_circle() {
        Ok(d2d.create_circle_geometry(circle)?.into())
    } else {
        path_from_shape(d2d, is_filled, shape, fill_rule)
    }
}

fn path_from_shape(
    d2d: &D2DFactory,
    is_filled: bool,
    shape: impl Shape,
    fill_rule: FillRule,
) -> Result<Geometry, Error> {
    let mut path = d2d.create_path_geometry()?;
    let mut sink = path.open()?;
    sink.set_fill_mode(fill_rule);
    let mut need_close = false;
    for el in shape.path_elements(BEZ_TOLERANCE) {
        match el {
            PathEl::MoveTo(p) => {
                if need_close {
                    sink.end_figure(false);
                }
                sink.begin_figure(to_point2f(p), is_filled);
                need_close = true;
            }
            PathEl::LineTo(p) => {
                sink.add_line(to_point2f(p));
            }
            PathEl::QuadTo(p1, p2) => {
                sink.add_quadratic_bezier(to_point2f(p1), to_point2f(p2));
            }
            PathEl::CurveTo(p1, p2, p3) => {
                sink.add_bezier(to_point2f(p1), to_point2f(p2), to_point2f(p3));
            }
            PathEl::ClosePath => {
                sink.end_figure(true);
                need_close = false;
            }
        }
    }
    if need_close {
        sink.end_figure(false);
    }
    sink.close()?;
    Ok(path.into())
}

impl<'a> RenderContext for D2DRenderContext<'a> {
    type Brush = Brush;

    type Text = D2DText;

    type TextLayout = D2DTextLayout;

    type Image = Bitmap;

    fn status(&mut self) -> Result<(), Error> {
        std::mem::replace(&mut self.err, Ok(()))
    }

    fn clear(&mut self, region: impl Into<Option<Rect>>, color: Color) {
        let old_transform = self.rt.get_transform();
        self.rt.set_transform_identity();

        // Remove clippings
        for _ in 0..self.layers.len() {
            self.rt.pop_layer();
        }
        if let Some(rect) = region.into() {
            self.rt.push_axis_aligned_clip(rect);
            self.rt.clear(color_to_colorf(color));
            self.rt.pop_axis_aligned_clip();
        } else {
            // Clear whole canvas
            self.rt.clear(color_to_colorf(color));
        }
        // Restore clippings
        for (mask, layer) in self.layers.iter() {
            self.rt.push_layer_mask(mask, layer);
        }

        self.rt.set_transform(&old_transform);
    }

    fn solid_brush(&mut self, color: Color) -> Brush {
        let device_context = &mut self.rt;
        let key = color.as_rgba_u32();
        self.brush_cache
            .entry(&key)
            .or_insert_with(
                || key,
                || {
                    device_context
                        .create_solid_color(color_to_colorf(color))
                        .expect("error creating solid brush")
                },
            )
            .clone()
    }

    fn gradient(&mut self, gradient: impl Into<FixedGradient>) -> Result<Brush, Error> {
        match gradient.into() {
            FixedGradient::Linear(linear) => {
                let props = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                    startPoint: to_point2f(linear.start),
                    endPoint: to_point2f(linear.end),
                };
                let stops: Vec<_> = linear.stops.iter().map(gradient_stop_to_d2d).collect();
                let stops = self.rt.create_gradient_stops(&stops)?;
                let result = self.rt.create_linear_gradient(&props, &stops)?;
                Ok(result)
            }
            FixedGradient::Radial(radial) => {
                let props = D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES {
                    center: to_point2f(radial.center),
                    gradientOriginOffset: to_point2f(radial.origin_offset),
                    radiusX: radial.radius as f32,
                    radiusY: radial.radius as f32,
                };
                let stops: Vec<_> = radial.stops.iter().map(gradient_stop_to_d2d).collect();
                let stops = self.rt.create_gradient_stops(&stops)?;
                let result = self.rt.create_radial_gradient(&props, &stops)?;
                Ok(result)
            }
        }
    }

    fn fill(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>) {
        self.fill_impl(shape, brush, FillRule::NonZero)
    }

    fn fill_even_odd(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>) {
        self.fill_impl(shape, brush, FillRule::EvenOdd)
    }

    fn stroke(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>, width: f64) {
        self.stroke_impl(shape, brush, width, None)
    }

    fn stroke_styled(
        &mut self,
        shape: impl Shape,
        brush: &impl IntoBrush<Self>,
        width: f64,
        style: &StrokeStyle,
    ) {
        let style = convert_stroke_style(self.factory, style, width)
            .expect("stroke style conversion failed");
        self.stroke_impl(shape, brush, width, Some(&style));
    }

    fn clip(&mut self, shape: impl Shape) {
        // TODO: set size based on bbox of shape.
        let layer = match self.rt.create_layer(None) {
            Ok(layer) => layer,
            Err(e) => {
                self.err = Err(e.into());
                return;
            }
        };
        let geom = match geometry_from_shape(self.factory, true, shape, FillRule::NonZero) {
            Ok(geom) => geom,
            Err(e) => {
                self.err = Err(e);
                return;
            }
        };
        self.rt.push_layer_mask(&geom, &layer);
        self.layers.push((geom, layer));
        self.ctx_stack.last_mut().unwrap().n_layers_pop += 1;
    }

    fn text(&mut self) -> &mut Self::Text {
        &mut self.inner_text
    }

    fn draw_text(&mut self, layout: &Self::TextLayout, pos: impl Into<Point>) {
        // TODO: bounding box for text
        layout.draw(pos.into(), self);
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
            return Err(Error::StackUnbalance);
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
            return Err(Error::StackUnbalance);
        }
        self.pop_state();
        std::mem::replace(&mut self.err, Ok(()))
    }

    fn transform(&mut self, transform: Affine) {
        self.ctx_stack.last_mut().unwrap().transform *= transform;
        self.rt
            .set_transform(&affine_to_matrix3x2f(self.current_transform()));
    }

    fn current_transform(&self) -> Affine {
        // This is an unwrap because we protect the invariant.
        self.ctx_stack.last().unwrap().transform
    }

    fn make_image(
        &mut self,
        width: usize,
        height: usize,
        buf: &[u8],
        format: ImageFormat,
    ) -> Result<Self::Image, Error> {
        // CreateBitmap will fail if we try to make an empty image. To solve this, we change an
        // empty image into 1x1 transparent image. Not ideal, but prevents a crash. TODO find a
        // better solution.
        if width == 0 || height == 0 {
            return Ok(self.rt.create_empty_bitmap()?);
        }

        // TODO: this method _really_ needs error checking, so much can go wrong...
        let alpha_mode = match format {
            ImageFormat::Rgb | ImageFormat::Grayscale => D2D1_ALPHA_MODE_IGNORE,
            ImageFormat::RgbaPremul | ImageFormat::RgbaSeparate => D2D1_ALPHA_MODE_PREMULTIPLIED,
            _ => return Err(Error::NotSupported),
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
            ImageFormat::Grayscale => {
                // it seems like there's no good way to create a 1-channel bitmap
                // here? I am not alone:
                // https://stackoverflow.com/questions/44270215/direct2d-fails-when-drawing-a-single-channel-bitmap
                let mut new_buf = vec![255; width * height * 4];
                for i in 0..width * height {
                    new_buf[i * 4 + 0] = buf[i];
                    new_buf[i * 4 + 1] = buf[i];
                    new_buf[i * 4 + 2] = buf[i];
                }
                Cow::from(new_buf)
            }
            // This should be unreachable, we caught it above.
            _ => return Err(Error::NotSupported),
        };
        let bitmap = self.rt.create_bitmap(width, height, &buf, alpha_mode)?;
        Ok(bitmap)
    }

    #[inline]
    fn draw_image(
        &mut self,
        image: &Self::Image,
        dst_rect: impl Into<Rect>,
        interp: InterpolationMode,
    ) {
        draw_image(self.rt, image, None, dst_rect.into(), interp);
    }

    #[inline]
    fn draw_image_area(
        &mut self,
        image: &Self::Image,
        src_rect: impl Into<Rect>,
        dst_rect: impl Into<Rect>,
        interp: InterpolationMode,
    ) {
        draw_image(
            self.rt,
            image,
            Some(src_rect.into()),
            dst_rect.into(),
            interp,
        );
    }

    fn blurred_rect(&mut self, rect: Rect, blur_radius: f64, brush: &impl IntoBrush<Self>) {
        let brush = brush.make_brush(self, || rect);
        if let Err(e) = self.blurred_rect_raw(rect, blur_radius, brush) {
            eprintln!("error in drawing blurred rect: {:?}", e);
        }
    }
}

impl<'a> D2DRenderContext<'a> {
    fn fill_impl(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>, fill_rule: FillRule) {
        let brush = brush.make_brush(self, || shape.bounding_box());

        // TODO: do something special (or nothing at all) for line?
        if let Some(rect) = shape.as_rect() {
            self.rt.fill_rect(rect, &brush)
        } else if let Some(round_rect) = shape
            .as_rounded_rect()
            .filter(|r| r.radii().as_single_radius().is_some())
        {
            self.rt.fill_rounded_rect(
                round_rect.rect(),
                round_rect.radii().as_single_radius().unwrap(),
                &brush,
            )
        } else if let Some(circle) = shape.as_circle() {
            self.rt.fill_circle(circle, &brush)
        } else {
            match path_from_shape(self.factory, true, shape, fill_rule) {
                Ok(geom) => self.rt.fill_geometry(&geom, &brush, None),
                Err(e) => self.err = Err(e),
            }
        }
    }

    fn stroke_impl(
        &mut self,
        shape: impl Shape,
        brush: &impl IntoBrush<Self>,
        width: f64,
        style: Option<&crate::d2d::StrokeStyle>,
    ) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        let width = width as f32;

        if let Some(line) = shape.as_line() {
            self.rt.draw_line(line, &brush, width, style);
            return;
        } else if let Some(rect) = shape.as_rect() {
            self.rt.draw_rect(rect, &brush, width, style);
            return;
        } else if let Some(round_rect) = shape.as_rounded_rect() {
            if let Some(radius) = round_rect.radii().as_single_radius() {
                self.rt
                    .draw_rounded_rect(round_rect.rect(), radius, &brush, width, style);
                return;
            }
        } else if let Some(circle) = shape.as_circle() {
            self.rt.draw_circle(circle, &brush, width, style);
            return;
        }

        let geom = match path_from_shape(self.factory, false, shape, FillRule::EvenOdd) {
            Ok(geom) => geom,
            Err(e) => {
                self.err = Err(e);
                return;
            }
        };
        let width = width;
        self.rt.draw_geometry(&geom, &*brush, width, style);
    }

    // This is split out to unify error reporting, as there are lots of opportunities for
    // errors in resource creation.
    fn blurred_rect_raw(
        &mut self,
        rect: Rect,
        blur_radius: f64,
        brush: Cow<Brush>,
    ) -> Result<(), Error> {
        let rect_exp = rect.expand();
        let widthf = rect_exp.width() as f32;
        let heightf = rect_exp.height() as f32;
        // Note: we're being fairly dumb about choosing the bitmap size, not taking
        // dpi scaling into account.
        let brt = self.rt.create_compatible_render_target(widthf, heightf)?;
        // Is it necessary to clear, or can we count on it being in a cleared
        // state when it's created?
        let clear_color = winapi::um::d2d1::D2D1_COLOR_F {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        };
        let draw_rect = rect_to_rectf(rect - rect_exp.origin().to_vec2());
        unsafe {
            brt.BeginDraw();
            brt.Clear(&clear_color);
            brt.FillRectangle(&draw_rect, brush.as_raw());
            let mut tag1 = 0;
            let mut tag2 = 0;
            let hr = brt.EndDraw(&mut tag1, &mut tag2);
            wrap_unit(hr)?;
        }
        // It might be slightly cleaner to create the effect on `brt`, but it should
        // be fine, as it's "compatible".
        let effect = self.rt.create_blur_effect(blur_radius)?;
        let bitmap = brt.get_bitmap()?;
        effect.set_input(0, bitmap.deref());
        let offset = to_point2f(rect_exp.origin());
        self.rt.draw_image_effect(
            &effect,
            Some(offset),
            None,
            D2D1_INTERPOLATION_MODE_LINEAR,
            D2D1_COMPOSITE_MODE_SOURCE_OVER,
        );
        Ok(())
    }
}

impl<'a> Drop for D2DRenderContext<'a> {
    fn drop(&mut self) {
        assert!(
            self.ctx_stack.is_empty(),
            "Render context dropped without finish() call"
        );
    }
}

fn draw_image<'a>(
    rt: &'a mut D2DDeviceContext,
    image: &<D2DRenderContext<'a> as RenderContext>::Image,
    src_rect: Option<Rect>,
    dst_rect: Rect,
    interp: InterpolationMode,
) {
    if dst_rect.is_empty() || image.empty_image {
        // source or destination are empty
        return;
    }
    let interp = match interp {
        InterpolationMode::NearestNeighbor => D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
        InterpolationMode::Bilinear => D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
    };
    let src_rect = match src_rect {
        Some(src_rect) => Some(rect_to_rectf(src_rect)),
        None => None,
    };
    rt.draw_bitmap(
        &image,
        &rect_to_rectf(dst_rect),
        1.0,
        interp,
        src_rect.as_ref(),
    );
}

impl<'a> IntoBrush<D2DRenderContext<'a>> for Brush {
    fn make_brush<'b>(
        &'b self,
        _piet: &mut D2DRenderContext,
        _bbox: impl FnOnce() -> Rect,
    ) -> std::borrow::Cow<'b, Brush> {
        Cow::Borrowed(self)
    }
}

impl Image for Bitmap {
    fn size(&self) -> Size {
        if self.empty_image {
            Size::ZERO
        } else {
            let inner = self.get_size();
            Size::new(inner.width.into(), inner.height.into())
        }
    }
}
