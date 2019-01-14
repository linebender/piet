//! The Web Canvas backend for the Piet 2D graphics abstraction.

use std::borrow::Cow;

use wasm_bindgen::{Clamped, JsCast, JsValue};
use web_sys::{CanvasRenderingContext2d, CanvasWindingRule, HtmlCanvasElement, ImageData, Window};

use kurbo::{Affine, PathEl, Rect, Shape, Vec2};

use piet::{
    Font, FontBuilder, InterpolationMode, RenderContext, RoundInto, TextLayout, TextLayoutBuilder,
};

pub struct WebRenderContext<'a> {
    ctx: &'a mut CanvasRenderingContext2d,
    /// Used for creating image bitmaps and possibly other resources.
    window: &'a Window,
}

impl<'a> WebRenderContext<'a> {
    pub fn new(ctx: &'a mut CanvasRenderingContext2d, window: &'a Window) -> WebRenderContext<'a> {
        WebRenderContext { ctx, window }
    }
}

pub enum Brush {
    Solid(u32),
}

pub enum StrokeStyle {
    // TODO: actual stroke style options
    Default,
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
    font: WebFont,
    text: String,
    width: f64,
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

fn convert_fill_rule(fill_rule: piet::FillRule) -> CanvasWindingRule {
    match fill_rule {
        piet::FillRule::NonZero => CanvasWindingRule::Nonzero,
        piet::FillRule::EvenOdd => CanvasWindingRule::Evenodd,
    }
}

impl<'a> RenderContext for WebRenderContext<'a> {
    /// wasm-bindgen doesn't have a native Point type, so use kurbo's.
    type Point = Vec2;
    type Coord = f64;
    type Brush = Brush;
    type StrokeStyle = StrokeStyle;

    type Font = WebFont;
    type FontBuilder = WebFontBuilder;
    type TextLayout = WebTextLayout;
    type TextLayoutBuilder = WebTextLayoutBuilder;

    type Image = WebImage;

    fn clear(&mut self, _rgb: u32) {
        // TODO: we might need to know the size of the canvas to do this.
    }

    fn solid_brush(&mut self, rgba: u32) -> Brush {
        Brush::Solid(rgba)
    }

    fn fill(&mut self, shape: impl Shape, brush: &Self::Brush, fill_rule: piet::FillRule) {
        self.set_path(shape);
        self.set_brush(brush, true);
        self.ctx
            .fill_with_canvas_winding_rule(convert_fill_rule(fill_rule));
    }

    fn clip(&mut self, shape: impl Shape, fill_rule: piet::FillRule) {
        self.set_path(shape);
        self.ctx
            .clip_with_canvas_winding_rule(convert_fill_rule(fill_rule));
    }

    fn stroke(
        &mut self,
        shape: impl Shape,
        brush: &Self::Brush,
        width: impl RoundInto<Self::Coord>,
        style: Option<&Self::StrokeStyle>,
    ) {
        self.set_path(shape);
        self.set_stroke(width.round_into(), style);
        self.set_brush(brush, false);
        self.ctx.stroke();
    }

    fn new_font_by_name(
        &mut self,
        name: &str,
        size: impl RoundInto<Self::Coord>,
    ) -> Self::FontBuilder {
        let font = WebFont {
            family: name.to_owned(),
            size: size.round_into(),
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

    fn draw_text(
        &mut self,
        layout: &Self::TextLayout,
        pos: impl RoundInto<Self::Point>,
        brush: &Self::Brush,
    ) {
        self.ctx.set_font(&layout.font.get_font_string());
        self.set_brush(brush, true);
        let pos = pos.round_into();
        // TODO: should we be tracking errors, or just ignoring them?
        let _ = self.ctx.fill_text(&layout.text, pos.x, pos.y);
    }

    fn save(&mut self) {
        self.ctx.save();
    }

    fn restore(&mut self) {
        self.ctx.restore();
    }

    fn finish(&mut self) {}

    fn transform(&mut self, transform: Affine) {
        let a = transform.as_coeffs();
        let _ = self.ctx.transform(a[0], a[1], a[2], a[3], a[4], a[5]);
    }

    fn make_rgba_image(&mut self, width: usize, height: usize, buf: &[u8]) -> Self::Image {
        let document = self.window.document().unwrap();
        let element = document.create_element("canvas").unwrap();
        let canvas = element.dyn_into::<HtmlCanvasElement>().unwrap();
        canvas.set_width(width as u32);
        canvas.set_height(height as u32);
        // Discussion topic: if buf were mut here, we could probably avoid this clone.
        let mut buf = buf.to_vec();
        let image_data =
            ImageData::new_with_u8_clamped_array(Clamped(&mut buf), width as u32).unwrap();
        let context = canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .unwrap();
        let _ = context.put_image_data(&image_data, 0.0, 0.0);
        WebImage {
            inner: canvas,
            width: width as u32,
            height: height as u32,
        }
    }

    fn draw_image(
        &mut self,
        image: &Self::Image,
        rect: impl Into<Rect>,
        _interp: InterpolationMode,
    ) {
        let rect = rect.into();
        self.ctx.save();
        // TODO: handle error
        let _ = self.ctx.translate(rect.x0, rect.y0);
        let _ = self.ctx.scale(
            rect.width() / (image.width as f64),
            rect.height() / (image.height as f64),
        );
        let _ = self
            .ctx
            .draw_image_with_html_canvas_element(&image.inner, 0.0, 0.0);
        self.ctx.restore();
    }
}

impl<'a> WebRenderContext<'a> {
    /// Set the source pattern to the brush.
    ///
    /// Cairo is super stateful, and we're trying to have more retained stuff.
    /// This is part of the impedance matching.
    fn set_brush(&mut self, brush: &Brush, is_fill: bool) {
        match *brush {
            Brush::Solid(rgba) => {
                let rgb = rgba >> 8;
                let a = rgba & 0xff;
                let color_str = if a == 0xff {
                    format!("#{:06x}", rgba >> 8)
                } else {
                    format!(
                        "rgba({},{},{},{:.3})",
                        (rgb >> 16) & 0xff,
                        (rgb >> 8) & 0xff,
                        rgb & 0xff,
                        byte_to_frac(a)
                    )
                };
                if is_fill {
                    self.ctx.set_fill_style(&JsValue::from_str(&color_str));
                } else {
                    self.ctx.set_stroke_style(&JsValue::from_str(&color_str));
                }
            }
        }
    }

    /// Set the stroke parameters.
    fn set_stroke(&mut self, width: f64, style: Option<&StrokeStyle>) {
        self.ctx.set_line_width(width);
        if let Some(style) = style {
            match style {
                // TODO: actual stroke style parameters
                StrokeStyle::Default => (),
            }
        }
    }

    fn set_path(&mut self, shape: impl Shape) {
        // This shouldn't be necessary, we always leave the context in no-path
        // state. But just in case, and it should be harmless.
        self.ctx.begin_path();
        for el in shape.to_bez_path(1e-3) {
            match el {
                PathEl::Moveto(p) => self.ctx.move_to(p.x, p.y),
                PathEl::Lineto(p) => self.ctx.line_to(p.x, p.y),
                PathEl::Quadto(p1, p2) => self.ctx.quadratic_curve_to(p1.x, p1.y, p2.x, p2.y),
                PathEl::Curveto(p1, p2, p3) => {
                    self.ctx.bezier_curve_to(p1.x, p1.y, p2.x, p2.y, p3.x, p3.y)
                }
                PathEl::Closepath => self.ctx.close_path(),
            }
        }
    }
}

fn byte_to_frac(byte: u32) -> f64 {
    ((byte & 255) as f64) * (1.0 / 255.0)
}

impl FontBuilder for WebFontBuilder {
    type Out = WebFont;

    fn build(self) -> Self::Out {
        self.0
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

    fn build(self) -> Self::Out {
        self.ctx.set_font(&self.font.get_font_string());
        let width = self
            .ctx
            .measure_text(&self.text)
            .map(|m| m.width())
            .unwrap_or(0.0);
        WebTextLayout {
            font: self.font,
            text: self.text,
            width,
        }
    }
}

impl TextLayout for WebTextLayout {
    type Coord = f64;

    fn width(&self) -> f64 {
        self.width
    }
}
