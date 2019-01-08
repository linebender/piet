//! The Web Canvas backend for the Piet 2D graphics abstraction.

use std::borrow::Cow;

use wasm_bindgen::JsValue;
use web_sys::{CanvasRenderingContext2d, CanvasWindingRule};

use kurbo::{PathEl, Shape, Vec2};

use piet::{Font, FontBuilder, RenderContext, RoundInto, TextLayout, TextLayoutBuilder};

pub struct WebRenderContext<'a> {
    ctx: &'a mut CanvasRenderingContext2d,
}

impl<'a> WebRenderContext<'a> {
    pub fn new(ctx: &mut CanvasRenderingContext2d) -> WebRenderContext {
        WebRenderContext { ctx }
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

/// https://developer.mozilla.org/en-US/docs/Web/CSS/font-style
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

    type F = WebFont;
    type FBuilder = WebFontBuilder;
    type TL = WebTextLayout;
    type TLBuilder = WebTextLayoutBuilder;

    fn clear(&mut self, _rgb: u32) {
        // TODO: we might need to know the size of the canvas to do this.
    }

    fn solid_brush(&mut self, rgba: u32) -> Brush {
        Brush::Solid(rgba)
    }

    fn fill(&mut self, shape: &impl Shape, brush: &Self::Brush, fill_rule: piet::FillRule) {
        self.set_path(shape);
        self.set_brush(brush, true);
        self.ctx
            .fill_with_canvas_winding_rule(convert_fill_rule(fill_rule));
    }

    fn stroke(
        &mut self,
        shape: &impl Shape,
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
    ) -> Self::FBuilder {
        let font = WebFont {
            family: name.to_owned(),
            size: size.round_into(),
            weight: 400,
            style: FontStyle::Normal,
        };
        WebFontBuilder(font)
    }

    fn new_text_layout(&mut self, font: &Self::F, text: &str) -> Self::TLBuilder {
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
        layout: &Self::TL,
        pos: impl RoundInto<Self::Point>,
        brush: &Self::Brush,
    ) {
        self.ctx.set_font(&layout.font.get_font_string());
        self.set_brush(brush, true);
        let pos = pos.round_into();
        // TODO: should we be tracking errors, or just ignoring them?
        let _ = self.ctx.fill_text(&layout.text, pos.x, pos.y);
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

    fn set_path(&mut self, shape: &impl Shape) {
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
