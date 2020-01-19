//! The Web Canvas backend for the Piet 2D graphics abstraction.
mod grapheme;

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
    Color, Error, FixedGradient, Font, FontBuilder, GradientStop, HitTestMetrics, HitTestPoint,
    HitTestTextPosition, ImageFormat, InterpolationMode, IntoBrush, LineCap, LineJoin,
    RenderContext, StrokeStyle, Text, TextLayout, TextLayoutBuilder,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::grapheme::point_x_in_grapheme;

pub struct WebRenderContext<'a> {
    ctx: &'a mut CanvasRenderingContext2d,
    /// Used for creating image bitmaps and possibly other resources.
    window: &'a Window,
    err: Result<(), Error>,
}

impl<'a> WebRenderContext<'a> {
    pub fn new(ctx: &'a mut CanvasRenderingContext2d, window: &'a Window) -> WebRenderContext<'a> {
        WebRenderContext {
            ctx,
            window,
            err: Ok(()),
        }
    }
}

#[derive(Clone)]
pub enum Brush {
    Solid(u32),
    Gradient(CanvasGradient),
}

#[derive(Clone)]
pub struct WebFont {
    family: String,
    weight: u32,
    style: FontStyle,
    size: f64,
}

pub struct WebFontBuilder(WebFont);

pub struct WebTextLayout {
    ctx: CanvasRenderingContext2d,
    font: WebFont,
    text: String,
}

pub struct WebTextLayoutBuilder {
    ctx: CanvasRenderingContext2d,
    font: WebFont,
    text: String,
}

pub struct WebImage {
    /// We use a canvas element for now, but could be ImageData or ImageBitmap,
    /// so consider an enum.
    inner: HtmlCanvasElement,
    width: u32,
    height: u32,
}

/// https://developer.mozilla.org/en-US/docs/Web/CSS/font-style
#[allow(dead_code)] // TODO: Remove
#[derive(Clone)]
enum FontStyle {
    Normal,
    Italic,
    Oblique(Option<f64>),
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
            Some(canvas) => (canvas.width(), canvas.height()),
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

    fn draw_image(
        &mut self,
        image: &Self::Image,
        rect: impl Into<Rect>,
        _interp: InterpolationMode,
    ) {
        let result = self.with_save(|rc| {
            let rect = rect.into();
            let _ = rc.ctx.translate(rect.x0, rect.y0);
            let _ = rc.ctx.scale(
                rect.width() / (image.width as f64),
                rect.height() / (image.height as f64),
            );
            rc.ctx
                .draw_image_with_html_canvas_element(&image.inner, 0.0, 0.0)
                .wrap()
        });
        if let Err(e) = result {
            self.err = Err(e);
        }
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

impl<'a> Text for WebRenderContext<'a> {
    type Font = WebFont;
    type FontBuilder = WebFontBuilder;
    type TextLayout = WebTextLayout;
    type TextLayoutBuilder = WebTextLayoutBuilder;

    fn new_font_by_name(&mut self, name: &str, size: f64) -> Self::FontBuilder {
        let font = WebFont {
            family: name.to_owned(),
            size,
            weight: 400,
            style: FontStyle::Normal,
        };
        WebFontBuilder(font)
    }

    fn new_text_layout(&mut self, font: &Self::Font, text: &str) -> Self::TextLayoutBuilder {
        WebTextLayoutBuilder {
            // TODO: it's very likely possible to do this without cloning ctx, but
            // I couldn't figure out the lifetime errors from a `&'a` reference.
            ctx: self.ctx.clone(),
            font: font.clone(),
            text: text.to_owned(),
        }
    }
}

impl<'a> WebRenderContext<'a> {
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

impl FontBuilder for WebFontBuilder {
    type Out = WebFont;

    fn build(self) -> Result<Self::Out, Error> {
        Ok(self.0)
    }
}

impl Font for WebFont {}

impl WebFont {
    fn get_font_string(&self) -> String {
        let style_str = match self.style {
            FontStyle::Normal => Cow::from("normal"),
            FontStyle::Italic => Cow::from("italic"),
            FontStyle::Oblique(None) => Cow::from("italic"),
            FontStyle::Oblique(Some(angle)) => Cow::from(format!("oblique {}deg", angle)),
        };
        format!(
            "{} {} {}px \"{}\"",
            style_str, self.weight, self.size, self.family
        )
    }
}

impl TextLayoutBuilder for WebTextLayoutBuilder {
    type Out = WebTextLayout;

    fn build(self) -> Result<Self::Out, Error> {
        self.ctx.set_font(&self.font.get_font_string());
        Ok(WebTextLayout {
            ctx: self.ctx,
            font: self.font,
            text: self.text,
        })
    }
}

impl TextLayout for WebTextLayout {
    fn width(&self) -> f64 {
        //cairo:
        //self.font.text_extents(&self.text).x_advance
        self.ctx
            .measure_text(&self.text)
            .map(|m| m.width())
            .expect("Text measurement failed")
    }

    // first assume one line.
    // TODO do with lines
    fn hit_test_point(&self, point: Point) -> HitTestPoint {
        // internal logic is using grapheme clusters, but return the text position associated
        // with the border of the grapheme cluster.

        // null case
        if self.text.len() == 0 {
            return HitTestPoint::default();
        }

        // get bounds
        // TODO handle if string is not null yet count is 0?
        let end = UnicodeSegmentation::graphemes(self.text.as_str(), true).count() - 1;
        let end_bounds = match self.get_grapheme_boundaries(end) {
            Some(bounds) => bounds,
            None => return HitTestPoint::default(),
        };

        let start = 0;
        let start_bounds = match self.get_grapheme_boundaries(start) {
            Some(bounds) => bounds,
            None => return HitTestPoint::default(),
        };

        // first test beyond ends
        if point.x > end_bounds.trailing {
            let mut res = HitTestPoint::default();
            res.metrics.text_position = self.text.len();
            return res;
        }
        if point.x <= start_bounds.leading {
            return HitTestPoint::default();
        }

        // then test the beginning and end (common cases)
        if let Some(hit) = point_x_in_grapheme(point.x, &start_bounds) {
            return hit;
        }
        if let Some(hit) = point_x_in_grapheme(point.x, &end_bounds) {
            return hit;
        }

        // Now that we know it's not beginning or end, begin binary search.
        // Iterative style
        let mut left = start;
        let mut right = end;
        loop {
            // pick halfway point
            let middle = left + ((right - left) / 2);

            let grapheme_bounds = match self.get_grapheme_boundaries(middle) {
                Some(bounds) => bounds,
                None => return HitTestPoint::default(),
            };

            if let Some(hit) = point_x_in_grapheme(point.x, &grapheme_bounds) {
                return hit;
            }

            // since it's not a hit, check if closer to start or finish
            // and move the appropriate search boundary
            if point.x < grapheme_bounds.leading {
                right = middle;
            } else if point.x > grapheme_bounds.trailing {
                left = middle + 1;
            } else {
                unreachable!("hit_test_point conditional is exhaustive");
            }
        }
    }

    fn hit_test_text_position(&self, text_position: usize) -> Option<HitTestTextPosition> {
        // Using substrings, but now with unicode grapheme awareness

        let text_len = self.text.len();

        if text_position == 0 {
            return Some(HitTestTextPosition::default());
        }

        if text_position as usize >= text_len {
            let x = self.width();

            return Some(HitTestTextPosition {
                point: Point { x, y: 0.0 },
                metrics: HitTestMetrics {
                    text_position: text_len,
                },
            });
        }

        // Already checked that text_position > 0 and text_position < count.
        // If text position is not at a grapheme boundary, use the text position of current
        // grapheme cluster. But return the original text position
        // Use the indices (byte offset, which for our purposes = utf8 code units).
        let grapheme_indices = UnicodeSegmentation::grapheme_indices(self.text.as_str(), true)
            .take_while(|(byte_idx, _s)| text_position >= *byte_idx);

        if let Some((byte_idx, _s)) = grapheme_indices.last() {
            let x = self
                .ctx
                .measure_text(&self.text[0..byte_idx])
                .map(|m| m.width())
                .expect("Text measurement failed");

            Some(HitTestTextPosition {
                point: Point { x, y: 0.0 },
                metrics: HitTestMetrics {
                    text_position: text_position,
                },
            })
        } else {
            // iterated to end boundary
            Some(HitTestTextPosition {
                point: Point {
                    x: self.width(),
                    y: 0.0,
                },
                metrics: HitTestMetrics {
                    text_position: text_len,
                },
            })
        }
    }
}
