// allows e.g. raw_data[dst_off + x * 4 + 2] = buf[src_off + x * 4 + 0];
#![allow(clippy::identity_op)]

//! The Web Canvas backend for the Piet 2D graphics abstraction.

mod text;

use std::borrow::Cow;
use std::fmt;
use std::ops::Deref;

use js_sys::{Float64Array, Reflect};
use wasm_bindgen::{Clamped, JsCast, JsValue};
use web_sys::{
    CanvasGradient, CanvasRenderingContext2d, CanvasWindingRule, HtmlCanvasElement, ImageData,
    Window,
};

use piet::kurbo::{Affine, PathEl, Point, Rect, Shape};

use piet::{
    Color, Error, FixedGradient, GradientStop, ImageFormat, InterpolationMode, IntoBrush, LineCap,
    LineJoin, RenderContext, StrokeStyle,
};

pub use text::{WebFont, WebFontBuilder, WebTextLayout, WebTextLayoutBuilder};

pub struct WebRenderContext<'a> {
    ctx: CanvasRenderingContext2d,
    /// Used for creating image bitmaps and possibly other resources.
    window: Window,
    err: Result<(), Error>,
    phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> WebRenderContext<'a> {
    pub fn new(ctx: CanvasRenderingContext2d, window: Window) -> WebRenderContext<'a> {
        WebRenderContext {
            ctx,
            window,
            err: Ok(()),
            phantom: std::marker::PhantomData,
        }
    }
}

#[derive(Clone)]
pub enum Brush {
    Solid(u32),
    Gradient(CanvasGradient),
}

pub struct WebImage {
    /// We use a canvas element for now, but could be ImageData or ImageBitmap,
    /// so consider an enum.
    inner: HtmlCanvasElement,
    width: u32,
    height: u32,
}

#[derive(Debug)]
struct WrappedJs(JsValue);

trait WrapError<T> {
    fn wrap(self) -> Result<T, Error>;
}

impl std::error::Error for WrappedJs {}

impl fmt::Display for WrappedJs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Canvas error: {:?}", self.0)
    }
}

// Discussion question: a blanket impl here should be pretty doable.

impl<T> WrapError<T> for Result<T, JsValue> {
    fn wrap(self) -> Result<T, Error> {
        self.map_err(|e| {
            let e: Box<dyn std::error::Error> = Box::new(WrappedJs(e));
            e.into()
        })
    }
}

fn convert_line_cap(line_cap: LineCap) -> &'static str {
    match line_cap {
        LineCap::Butt => "butt",
        LineCap::Round => "round",
        LineCap::Square => "square",
    }
}

fn convert_line_join(line_join: LineJoin) -> &'static str {
    match line_join {
        LineJoin::Miter => "miter",
        LineJoin::Round => "round",
        LineJoin::Bevel => "bevel",
    }
}

impl<'a> RenderContext for WebRenderContext<'a> {
    /// wasm-bindgen doesn't have a native Point type, so use kurbo's.
    type Brush = Brush;

    type Text = Self;
    type TextLayout = WebTextLayout;

    type Image = WebImage;

    fn status(&mut self) -> Result<(), Error> {
        std::mem::replace(&mut self.err, Ok(()))
    }

    fn clear(&mut self, color: Color) {
        let (width, height) = match self.ctx.canvas() {
            Some(canvas) => (canvas.offset_width(), canvas.offset_height()),
            None => return,
            /* Canvas might be null if the dom node is not in
             * the document; do nothing. */
        };
        let shape = Rect::new(0.0, 0.0, width as f64, height as f64);
        let brush = self.solid_brush(color);
        self.fill(shape, &brush);
    }

    fn solid_brush(&mut self, color: Color) -> Brush {
        Brush::Solid(color.as_rgba_u32())
    }

    fn gradient(&mut self, gradient: impl Into<FixedGradient>) -> Result<Brush, Error> {
        match gradient.into() {
            FixedGradient::Linear(linear) => {
                let (x0, y0) = (linear.start.x, linear.start.y);
                let (x1, y1) = (linear.end.x, linear.end.y);
                let mut lg = self.ctx.create_linear_gradient(x0, y0, x1, y1);
                set_gradient_stops(&mut lg, &linear.stops);
                Ok(Brush::Gradient(lg))
            }
            FixedGradient::Radial(radial) => {
                let (xc, yc) = (radial.center.x, radial.center.y);
                let (xo, yo) = (radial.origin_offset.x, radial.origin_offset.y);
                let r = radial.radius;
                let mut rg = self
                    .ctx
                    .create_radial_gradient(xc + xo, yc + yo, 0.0, xc, yc, r)
                    .wrap()?;
                set_gradient_stops(&mut rg, &radial.stops);
                Ok(Brush::Gradient(rg))
            }
        }
    }

    fn fill(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        self.set_path(shape);
        self.set_brush(&*brush, true);
        self.ctx
            .fill_with_canvas_winding_rule(CanvasWindingRule::Nonzero);
    }

    fn fill_even_odd(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        self.set_path(shape);
        self.set_brush(&*brush, true);
        self.ctx
            .fill_with_canvas_winding_rule(CanvasWindingRule::Evenodd);
    }

    fn clip(&mut self, shape: impl Shape) {
        self.set_path(shape);
        self.ctx
            .clip_with_canvas_winding_rule(CanvasWindingRule::Nonzero);
    }

    fn stroke(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>, width: f64) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        self.set_path(shape);
        self.set_stroke(width, None);
        self.set_brush(&*brush.deref(), false);
        self.ctx.stroke();
    }

    fn stroke_styled(
        &mut self,
        shape: impl Shape,
        brush: &impl IntoBrush<Self>,
        width: f64,
        style: &StrokeStyle,
    ) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        self.set_path(shape);
        self.set_stroke(width, Some(style));
        self.set_brush(&*brush.deref(), false);
        self.ctx.stroke();
    }

    fn text(&mut self) -> &mut Self::Text {
        self
    }

    fn draw_text(
        &mut self,
        layout: &Self::TextLayout,
        pos: impl Into<Point>,
        brush: &impl IntoBrush<Self>,
    ) {
        // TODO: bounding box for text
        let brush = brush.make_brush(self, || Rect::ZERO);
        self.ctx.set_font(&layout.font.get_font_string());
        self.set_brush(&*brush, true);
        let pos = pos.into();
        if let Err(e) = self.ctx.fill_text(&layout.text, pos.x, pos.y).wrap() {
            self.err = Err(e);
        }
    }

    fn save(&mut self) -> Result<(), Error> {
        self.ctx.save();
        Ok(())
    }

    fn restore(&mut self) -> Result<(), Error> {
        self.ctx.restore();
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Error> {
        self.status()
    }

    fn transform(&mut self, transform: Affine) {
        let a = transform.as_coeffs();
        let _ = self.ctx.transform(a[0], a[1], a[2], a[3], a[4], a[5]);
    }

    fn current_transform(&self) -> Affine {
        // todo
        // current_transform() and get_transform() currently not implemented:
        // https://github.com/rustwasm/wasm-bindgen/blob/f8354b3a88de013845a304ea77d8b9b9286a0d7b/crates/web-sys/webidls/enabled/CanvasRenderingContext2D.webidl#L136
        Affine::default()
    }

    fn make_image(
        &mut self,
        width: usize,
        height: usize,
        buf: &[u8],
        format: ImageFormat,
    ) -> Result<Self::Image, Error> {
        let document = self.window.document().unwrap();
        let element = document.create_element("canvas").unwrap();
        let canvas = element.dyn_into::<HtmlCanvasElement>().unwrap();
        canvas.set_width(width as u32);
        canvas.set_height(height as u32);
        let mut buf = match format {
            // Discussion topic: if buf were mut here, we could probably avoid this clone.
            // See https://github.com/rustwasm/wasm-bindgen/issues/1005 for an issue that might
            // also resolve the need to clone.
            ImageFormat::RgbaSeparate => buf.to_vec(),
            ImageFormat::RgbaPremul => {
                fn unpremul(x: u8, a: u8) -> u8 {
                    if a == 0 {
                        0
                    } else {
                        let y = (x as u32 * 255 + (a as u32 / 2)) / (a as u32);
                        y.min(255) as u8
                    }
                }
                let mut new_buf = vec![0; width * height * 4];
                for i in 0..width * height {
                    let a = buf[i * 4 + 3];
                    new_buf[i * 4 + 0] = unpremul(buf[i * 4 + 0], a);
                    new_buf[i * 4 + 1] = unpremul(buf[i * 4 + 1], a);
                    new_buf[i * 4 + 2] = unpremul(buf[i * 4 + 2], a);
                    new_buf[i * 4 + 3] = a;
                }
                new_buf
            }
            ImageFormat::Rgb => {
                let mut new_buf = vec![0; width * height * 4];
                for i in 0..width * height {
                    new_buf[i * 4 + 0] = buf[i * 3 + 0];
                    new_buf[i * 4 + 1] = buf[i * 3 + 1];
                    new_buf[i * 4 + 2] = buf[i * 3 + 2];
                    new_buf[i * 4 + 3] = 255;
                }
                new_buf
            }
            _ => Vec::new(),
        };
        let image_data =
            ImageData::new_with_u8_clamped_array(Clamped(&mut buf), width as u32).wrap()?;
        let context = canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .unwrap();
        context.put_image_data(&image_data, 0.0, 0.0).wrap()?;
        Ok(WebImage {
            inner: canvas,
            width: width as u32,
            height: height as u32,
        })
    }

    #[inline]
    fn draw_image(
        &mut self,
        image: &Self::Image,
        dst_rect: impl Into<Rect>,
        interp: InterpolationMode,
    ) {
        draw_image(self, image, None, dst_rect.into(), interp);
    }

    #[inline]
    fn draw_image_area(
        &mut self,
        image: &Self::Image,
        src_rect: impl Into<Rect>,
        dst_rect: impl Into<Rect>,
        interp: InterpolationMode,
    ) {
        draw_image(self, image, Some(src_rect.into()), dst_rect.into(), interp);
    }
}

fn draw_image(
    ctx: &mut WebRenderContext,
    image: &<WebRenderContext as RenderContext>::Image,
    src_rect: Option<Rect>,
    dst_rect: Rect,
    _interp: InterpolationMode,
) {
    let result = ctx.with_save(|rc| {
        // TODO: Implement InterpolationMode::NearestNeighbor in software
        //       See for inspiration http://phrogz.net/tmp/canvas_image_zoom.html
        let src_rect = match src_rect {
            Some(src_rect) => src_rect,
            None => Rect::new(0.0, 0.0, image.width as f64, image.height as f64),
        };
        rc.ctx
            .draw_image_with_html_canvas_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                &image.inner,
                src_rect.x0,
                src_rect.y0,
                src_rect.width(),
                src_rect.height(),
                dst_rect.x0,
                dst_rect.y0,
                dst_rect.width(),
                dst_rect.height(),
            )
            .wrap()
    });
    if let Err(e) = result {
        ctx.err = Err(e);
    }
}

impl<'a> IntoBrush<WebRenderContext<'a>> for Brush {
    fn make_brush<'b>(
        &'b self,
        _piet: &mut WebRenderContext,
        _bbox: impl FnOnce() -> Rect,
    ) -> std::borrow::Cow<'b, Brush> {
        Cow::Borrowed(self)
    }
}

fn format_color(rgba: u32) -> String {
    let rgb = rgba >> 8;
    let a = rgba & 0xff;
    if a == 0xff {
        format!("#{:06x}", rgba >> 8)
    } else {
        format!(
            "rgba({},{},{},{:.3})",
            (rgb >> 16) & 0xff,
            (rgb >> 8) & 0xff,
            rgb & 0xff,
            byte_to_frac(a)
        )
    }
}

fn set_gradient_stops(dst: &mut CanvasGradient, src: &[GradientStop]) {
    for stop in src {
        // TODO: maybe get error?
        let rgba = stop.color.as_rgba_u32();
        let _ = dst.add_color_stop(stop.pos, &format_color(rgba));
    }
}

impl WebRenderContext<'_> {
    /// Set the source pattern to the brush.
    ///
    /// Web canvas is super stateful, and we're trying to have more retained stuff.
    /// This is part of the impedance matching.
    fn set_brush(&mut self, brush: &Brush, is_fill: bool) {
        match *brush {
            Brush::Solid(rgba) => {
                let color_str = format_color(rgba);
                if is_fill {
                    self.ctx.set_fill_style(&JsValue::from_str(&color_str));
                } else {
                    self.ctx.set_stroke_style(&JsValue::from_str(&color_str));
                }
            }
            Brush::Gradient(ref gradient) => {
                if is_fill {
                    self.ctx.set_fill_style(&JsValue::from(gradient));
                } else {
                    self.ctx.set_stroke_style(&JsValue::from(gradient));
                }
            }
        }
    }

    /// Set the stroke parameters.
    ///
    /// TODO(performance): this is probably expensive enough it makes sense
    /// to at least store the last version and only reset if it's changed.
    fn set_stroke(&mut self, width: f64, style: Option<&StrokeStyle>) {
        self.ctx.set_line_width(width);

        let line_join = style
            .and_then(|style| style.line_join)
            .unwrap_or(LineJoin::Miter);
        self.ctx.set_line_join(convert_line_join(line_join));

        let line_cap = style
            .and_then(|style| style.line_cap)
            .unwrap_or(LineCap::Butt);
        self.ctx.set_line_cap(convert_line_cap(line_cap));

        let miter_limit = style.and_then(|style| style.miter_limit).unwrap_or(10.0);
        self.ctx.set_miter_limit(miter_limit);

        let (dash_segs, dash_offset) = style
            .and_then(|style| style.dash.as_ref())
            .map(|dash| {
                let len = dash.0.len() as u32;
                let array = Float64Array::new_with_length(len);
                for (i, elem) in dash.0.iter().enumerate() {
                    Reflect::set(
                        array.as_ref(),
                        &JsValue::from(i as u32),
                        &JsValue::from(*elem),
                    )
                    .unwrap();
                }
                (array, dash.1)
            })
            .unwrap_or((Float64Array::new_with_length(0), 0.0));

        self.ctx.set_line_dash(dash_segs.as_ref()).unwrap();
        self.ctx.set_line_dash_offset(dash_offset);
    }

    fn set_path(&mut self, shape: impl Shape) {
        // This shouldn't be necessary, we always leave the context in no-path
        // state. But just in case, and it should be harmless.
        self.ctx.begin_path();
        for el in shape.to_bez_path(1e-3) {
            match el {
                PathEl::MoveTo(p) => self.ctx.move_to(p.x, p.y),
                PathEl::LineTo(p) => self.ctx.line_to(p.x, p.y),
                PathEl::QuadTo(p1, p2) => self.ctx.quadratic_curve_to(p1.x, p1.y, p2.x, p2.y),
                PathEl::CurveTo(p1, p2, p3) => {
                    self.ctx.bezier_curve_to(p1.x, p1.y, p2.x, p2.y, p3.x, p3.y)
                }
                PathEl::ClosePath => self.ctx.close_path(),
            }
        }
    }
}

fn byte_to_frac(byte: u32) -> f64 {
    ((byte & 255) as f64) * (1.0 / 255.0)
}
