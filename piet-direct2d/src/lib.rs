#![cfg(windows)]
//! The Direct2D backend for the Piet 2D graphics abstraction.

mod conv;
pub mod d2d;
pub mod d3d;
pub mod dwrite;

pub use d2d::{D2DDevice, D2DFactory, DeviceContext as D2DDeviceContext};
pub use dwrite::DwriteFactory;

use crate::conv::{
    affine_to_matrix3x2f, color_to_colorf, convert_stroke_style, gradient_stop_to_d2d,
    rect_to_rectf, to_point2f,
};

use std::borrow::Cow;
use std::convert::TryInto;

use winapi::um::d2d1::{
    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
    D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES,
    D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES,
};
use winapi::um::dcommon::{D2D1_ALPHA_MODE_IGNORE, D2D1_ALPHA_MODE_PREMULTIPLIED};

use piet::kurbo::{Affine, PathEl, Point, Rect, Shape};

use piet::{
    new_error, Color, Error, ErrorKind, FixedGradient, Font, FontBuilder, HitTestMetrics,
    HitTestPoint, HitTestTextPosition, ImageFormat, InterpolationMode, IntoBrush, RenderContext,
    StrokeStyle, Text, TextLayout, TextLayoutBuilder,
};

use d2d::{Bitmap, Brush, DeviceContext, FillRule, PathGeometry};
use dwrite::{TextFormat, TextFormatBuilder};

pub struct D2DRenderContext<'a> {
    factory: &'a D2DFactory,
    inner_text: D2DText<'a>,
    rt: &'a mut D2DDeviceContext,

    /// The context state stack. There is always at least one, until finishing.
    ctx_stack: Vec<CtxState>,

    err: Result<(), Error>,
}

pub struct D2DText<'a> {
    dwrite: &'a DwriteFactory,
}

pub struct D2DFont(TextFormat);

pub struct D2DFontBuilder<'a> {
    builder: TextFormatBuilder<'a>,
    name: String,
}

pub struct D2DTextLayout {
    text: String,
    layout: dwrite::TextLayout,
}

pub struct D2DTextLayoutBuilder<'a> {
    text: String,
    builder: dwrite::TextLayoutBuilder<'a>,
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
        dwrite: &'a DwriteFactory,
        rt: &'b mut DeviceContext,
    ) -> D2DRenderContext<'b> {
        let inner_text = D2DText { dwrite };
        D2DRenderContext {
            factory,
            inner_text: inner_text,
            rt,
            ctx_stack: vec![CtxState::default()],
            err: Ok(()),
        }
    }

    fn pop_state(&mut self) {
        // This is an unwrap because we protect the invariant.
        let old_state = self.ctx_stack.pop().unwrap();
        for _ in 0..old_state.n_layers_pop {
            self.rt.pop_layer();
        }
    }
}

// The setting of 1e-3 is extremely conservative (absolutely no
// differences should be visible) but setting a looser tolerance is
// likely a tiny performance improvement. We could fine-tune based on
// empirical study of both quality and performance.
const BEZ_TOLERANCE: f64 = 1e-3;

fn path_from_shape(
    d2d: &D2DFactory,
    is_filled: bool,
    shape: impl Shape,
    fill_rule: FillRule,
) -> Result<PathGeometry, Error> {
    let mut path = d2d.create_path_geometry()?;
    let mut sink = path.open()?;
    sink.set_fill_mode(fill_rule);
    let mut need_close = false;
    for el in shape.to_bez_path(BEZ_TOLERANCE) {
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
    Ok(path)
}

impl<'a> RenderContext for D2DRenderContext<'a> {
    type Brush = Brush;

    type Text = D2DText<'a>;

    type TextLayout = D2DTextLayout;

    type Image = Bitmap;

    fn status(&mut self) -> Result<(), Error> {
        std::mem::replace(&mut self.err, Ok(()))
    }

    fn clear(&mut self, color: Color) {
        self.rt.clear(color_to_colorf(color));
    }

    fn solid_brush(&mut self, color: Color) -> Brush {
        self.rt
            .create_solid_color(color_to_colorf(color))
            .expect("error creating solid brush")
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
        // TODO: various special-case shapes, for efficiency
        let brush = brush.make_brush(self, || shape.bounding_box());
        match path_from_shape(self.factory, true, shape, FillRule::NonZero) {
            Ok(path) => self.rt.fill_geometry(&path, &brush, None),
            Err(e) => self.err = Err(e),
        }
    }

    fn fill_even_odd(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>) {
        // TODO: various special-case shapes, for efficiency
        let brush = brush.make_brush(self, || shape.bounding_box());
        match path_from_shape(self.factory, true, shape, FillRule::EvenOdd) {
            Ok(path) => self.rt.fill_geometry(&path, &brush, None),
            Err(e) => self.err = Err(e),
        }
    }

    fn stroke(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>, width: f64) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        // TODO: various special-case shapes, for efficiency
        let path = match path_from_shape(self.factory, false, shape, FillRule::EvenOdd) {
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
        let path = match path_from_shape(self.factory, false, shape, FillRule::EvenOdd) {
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
        let layer = match self.rt.create_layer(None) {
            Ok(layer) => layer,
            Err(e) => {
                self.err = Err(e.into());
                return;
            }
        };
        let path = match path_from_shape(self.factory, true, shape, FillRule::NonZero) {
            Ok(path) => path,
            Err(e) => {
                self.err = Err(e);
                return;
            }
        };
        self.rt.push_layer_mask(&path, &layer);
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
        let mut pos = to_point2f(pos.into());
        pos.y -= line_metrics[0].baseline;
        let text_options = D2D1_DRAW_TEXT_OPTIONS_NONE;

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
        // TODO: this method _really_ needs error checking, so much can go wrong...
        let alpha_mode = match format {
            ImageFormat::Rgb => D2D1_ALPHA_MODE_IGNORE,
            ImageFormat::RgbaPremul | ImageFormat::RgbaSeparate => D2D1_ALPHA_MODE_PREMULTIPLIED,
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
        let bitmap = self.rt.create_bitmap(width, height, &buf, alpha_mode)?;
        Ok(bitmap)
    }

    fn draw_image(
        &mut self,
        image: &Self::Image,
        rect: impl Into<Rect>,
        interp: InterpolationMode,
    ) {
        let interp = match interp {
            InterpolationMode::NearestNeighbor => D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
            InterpolationMode::Bilinear => D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
        };
        self.rt
            .draw_bitmap(&image, &rect_to_rectf(rect.into()), 1.0, interp, None);
    }
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

impl<'a> D2DText<'a> {
    /// Create a new factory that satisfies the piet `Text` trait given
    /// the (platform-specific) dwrite factory.
    pub fn new(dwrite: &'a DwriteFactory) -> D2DText<'a> {
        D2DText { dwrite }
    }
}

impl<'a> Text for D2DText<'a> {
    type FontBuilder = D2DFontBuilder<'a>;
    type Font = D2DFont;
    type TextLayoutBuilder = D2DTextLayoutBuilder<'a>;
    type TextLayout = D2DTextLayout;

    fn new_font_by_name(&mut self, name: &str, size: f64) -> Self::FontBuilder {
        // Note: the name is cloned here, rather than applied using `with_family` for
        // lifetime reasons. Maybe there's a better approach.
        let builder = TextFormatBuilder::new(self.dwrite).size(size as f32);
        D2DFontBuilder {
            builder,
            name: name.to_owned(),
        }
    }

    fn new_text_layout(&mut self, font: &Self::Font, text: &str) -> Self::TextLayoutBuilder {
        D2DTextLayoutBuilder {
            text: text.to_owned(),
            builder: dwrite::TextLayoutBuilder::new(self.dwrite)
                .format(&font.0)
                .text(text),
        }
    }
}

impl<'a> FontBuilder for D2DFontBuilder<'a> {
    type Out = D2DFont;

    fn build(self) -> Result<Self::Out, Error> {
        Ok(D2DFont(self.builder.family(&self.name).build()?))
    }
}

impl Font for D2DFont {}

impl<'a> TextLayoutBuilder for D2DTextLayoutBuilder<'a> {
    type Out = D2DTextLayout;

    fn build(self) -> Result<Self::Out, Error> {
        Ok(D2DTextLayout {
            text: self.text,
            layout: self
                .builder
                .width(1e6) // TODO: probably want to support wrapping
                .height(1e6)
                .build()?,
        })
    }
}

impl TextLayout for D2DTextLayout {
    fn width(&self) -> f64 {
        self.layout.get_metrics().width as f64
    }

    fn hit_test_point(&self, point: Point) -> HitTestPoint {
        // lossy from f64 to f32, but shouldn't have too much impact
        let htp = self.layout.hit_test_point(point.x as f32, point.y as f32);

        // Round up to next grapheme cluster boundary if directwrite
        // reports a trailing hit.
        let text_position_16 = if htp.is_trailing_hit {
            htp.metrics.text_position + htp.metrics.length
        } else {
            htp.metrics.text_position
        } as usize;

        // Convert text position from utf-16 code units to
        // utf-8 code units.
        // Strategy: count up in utf16 and utf8 simultaneously, stop when
        // utf-16 text position reached.
        //
        // TODO ask about text_position, it looks like windows returns last index;
        // can't use the text_position of last index from directwrite, it has an extra code unit.
        let text_position =
            count_until_utf16(&self.text, text_position_16).unwrap_or(self.text.len());

        HitTestPoint {
            metrics: HitTestMetrics { text_position },
            is_inside: htp.is_inside,
        }
    }

    // Can panic if text position is not at a code point boundary, or if it's out of bounds.
    fn hit_test_text_position(&self, text_position: usize) -> Option<HitTestTextPosition> {
        // Note: Directwrite will just return the line width if text position is
        // out of bounds. This is what want for piet; return line width for the last text position
        // (equal to line.len()). This is basically returning line width for the last cursor
        // position.

        // Now convert the utf8 index to utf16.
        // This can panic;
        let idx_16 = count_utf16(&self.text[0..text_position]);

        // panic or Result are also fine options for dealing with overflow. Using Option here
        // because it's already present and convenient.
        // TODO this should probably go before convertin to utf16, since that's relatively slow
        let idx_16 = idx_16.try_into().ok()?;

        // TODO quick fix until directwrite fixes bool bug
        let trailing = true;

        self.layout
            .hit_test_text_position(idx_16, trailing)
            .map(|http| {
                HitTestTextPosition {
                    point: Point {
                        x: http.point_x as f64,
                        y: http.point_y as f64,
                    },
                    metrics: HitTestMetrics {
                        text_position, // no need to use directwrite return value
                    },
                }
            })
    }
}

/// Counts the number of utf-16 code units in the given string.
/// from xi-editor
pub(crate) fn count_utf16(s: &str) -> usize {
    let mut utf16_count = 0;
    for &b in s.as_bytes() {
        if (b as i8) >= -0x40 {
            utf16_count += 1;
        }
        if b >= 0xf0 {
            utf16_count += 1;
        }
    }
    utf16_count
}

/// returns utf8 text position (code unit offset)
/// at the given utf-16 text position
pub(crate) fn count_until_utf16(s: &str, utf16_text_position: usize) -> Option<usize> {
    let mut utf8_count = 0;
    let mut utf16_count = 0;
    for &b in s.as_bytes() {
        if (b as i8) >= -0x40 {
            utf16_count += 1;
        }
        if b >= 0xf0 {
            utf16_count += 1;
        }

        if utf16_count > utf16_text_position {
            return Some(utf8_count);
        }

        utf8_count += 1;
    }

    None
}

#[cfg(test)]
mod test {
    use crate::*;
    use piet::TextLayout;

    // - x: calculated value
    // - target: f64
    // - tolerance: in f64
    fn assert_close_to(x: f64, target: f64, tolerance: f64) {
        let min = target - tolerance;
        let max = target + tolerance;
        assert!(x <= max && x >= min);
    }

    #[test]
    fn test_hit_test_text_position_basic() {
        let dwrite = DwriteFactory::new().unwrap();
        let mut text_layout = D2DText::new(&dwrite);

        let input = "piet text!";
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();

        let layout = text_layout
            .new_text_layout(&font, &input[0..4])
            .build()
            .unwrap();
        let piet_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, &input[0..3])
            .build()
            .unwrap();
        let pie_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, &input[0..2])
            .build()
            .unwrap();
        let pi_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, &input[0..1])
            .build()
            .unwrap();
        let p_width = layout.width();

        let layout = text_layout.new_text_layout(&font, "").build().unwrap();
        let null_width = layout.width();

        let full_layout = text_layout.new_text_layout(&font, input).build().unwrap();
        let full_width = full_layout.width();

        assert_close_to(
            full_layout.hit_test_text_position(4).unwrap().point.x as f64,
            piet_width,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(3).unwrap().point.x as f64,
            pie_width,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(2).unwrap().point.x as f64,
            pi_width,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(1).unwrap().point.x as f64,
            p_width,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(0).unwrap().point.x as f64,
            null_width,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(10).unwrap().point.x as f64,
            full_width,
            3.0,
        );
    }

    #[test]
    fn test_hit_test_text_position_complex_0() {
        let dwrite = DwriteFactory::new().unwrap();

        let input = "é";
        assert_eq!(input.len(), 2);

        let mut text_layout = D2DText::new(&dwrite);
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout.new_text_layout(&font, input).build().unwrap();

        assert_close_to(layout.hit_test_text_position(0).unwrap().point.x, 0.0, 3.0);
        assert_close_to(
            layout.hit_test_text_position(2).unwrap().point.x,
            layout.width(),
            3.0,
        );

        // unicode segmentation is wrong on this one for now.
        //let input = "🤦\u{1f3fc}\u{200d}\u{2642}\u{fe0f}";

        //let mut text_layout = D2DText::new();
        //let font = text_layout.new_font_by_name("sans-serif", 12.0).build().unwrap();
        //let layout = text_layout.new_text_layout(&font, input).build().unwrap();

        //assert_eq!(input.graphemes(true).count(), 1);
        //assert_eq!(layout.hit_test_text_position(0, true).map(|p| p.point_x as f64), Some(layout.width()));
        //assert_eq!(input.len(), 17);

        let input = "\u{0023}\u{FE0F}\u{20E3}"; // #️⃣
        assert_eq!(input.len(), 7);
        assert_eq!(input.chars().count(), 3);

        let mut text_layout = D2DText::new(&dwrite);
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout.new_text_layout(&font, input).build().unwrap();

        assert_close_to(layout.hit_test_text_position(0).unwrap().point.x, 0.0, 3.0);
        assert_close_to(
            layout.hit_test_text_position(7).unwrap().point.x,
            layout.width(),
            3.0,
        );

        // note code unit not at grapheme boundary
        assert_close_to(layout.hit_test_text_position(1).unwrap().point.x, 0.0, 3.0);
        assert_eq!(
            layout
                .hit_test_text_position(1)
                .unwrap()
                .metrics
                .text_position,
            1
        );
    }

    #[test]
    fn test_hit_test_text_position_complex_1() {
        let dwrite = DwriteFactory::new().unwrap();

        // Notes on this input:
        // 6 code points
        // 7 utf-16 code units (1/1/1/1/1/2)
        // 14 utf-8 code units (2/1/3/3/1/4)
        // 4 graphemes
        let input = "é\u{0023}\u{FE0F}\u{20E3}1\u{1D407}"; // #️⃣,, 𝐇
        assert_eq!(input.len(), 14);

        let mut text_layout = D2DText::new(&dwrite);
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout.new_text_layout(&font, input).build().unwrap();

        let test_layout_0 = text_layout
            .new_text_layout(&font, &input[0..2])
            .build()
            .unwrap();
        let test_layout_1 = text_layout
            .new_text_layout(&font, &input[0..9])
            .build()
            .unwrap();
        let test_layout_2 = text_layout
            .new_text_layout(&font, &input[0..10])
            .build()
            .unwrap();

        // Note: text position is in terms of utf8 code units
        assert_close_to(layout.hit_test_text_position(0).unwrap().point.x, 0.0, 3.0);
        assert_close_to(
            layout.hit_test_text_position(2).unwrap().point.x,
            test_layout_0.width(),
            3.0,
        );
        assert_close_to(
            layout.hit_test_text_position(9).unwrap().point.x,
            test_layout_1.width(),
            3.0,
        );
        assert_close_to(
            layout.hit_test_text_position(10).unwrap().point.x,
            test_layout_2.width(),
            3.0,
        );
        assert_close_to(
            layout.hit_test_text_position(14).unwrap().point.x,
            layout.width(),
            3.0,
        );

        // Code point boundaries, but not grapheme boundaries.
        // Width should stay at the current grapheme boundary.
        assert_close_to(
            layout.hit_test_text_position(3).unwrap().point.x,
            test_layout_0.width(),
            3.0,
        );
        assert_eq!(
            layout
                .hit_test_text_position(3)
                .unwrap()
                .metrics
                .text_position,
            3
        );
        assert_close_to(
            layout.hit_test_text_position(6).unwrap().point.x,
            test_layout_0.width(),
            3.0,
        );
        assert_eq!(
            layout
                .hit_test_text_position(6)
                .unwrap()
                .metrics
                .text_position,
            6
        );
    }

    #[test]
    fn test_hit_test_point_basic() {
        let dwrite = DwriteFactory::new().unwrap();

        let mut text_layout = D2DText::new(&dwrite);

        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout
            .new_text_layout(&font, "piet text!")
            .build()
            .unwrap();
        println!("text pos 4: {:?}", layout.hit_test_text_position(4)); // 20.302734375
        println!("text pos 5: {:?}", layout.hit_test_text_position(5)); // 23.58984375

        // test hit test point
        // all inside
        let pt = layout.hit_test_point(Point::new(21.0, 0.0));
        assert_eq!(pt.metrics.text_position, 4);
        let pt = layout.hit_test_point(Point::new(22.0, 0.0));
        assert_eq!(pt.metrics.text_position, 5);
        let pt = layout.hit_test_point(Point::new(23.0, 0.0));
        assert_eq!(pt.metrics.text_position, 5);
        let pt = layout.hit_test_point(Point::new(24.0, 0.0));
        assert_eq!(pt.metrics.text_position, 5);
        let pt = layout.hit_test_point(Point::new(25.0, 0.0));
        assert_eq!(pt.metrics.text_position, 5);

        // outside
        println!("layout_width: {:?}", layout.width()); // 46.916015625

        let pt = layout.hit_test_point(Point::new(48.0, 0.0));
        assert_eq!(pt.metrics.text_position, 10); // last text position
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(-1.0, 0.0));
        assert_eq!(pt.metrics.text_position, 0); // first text position
        assert_eq!(pt.is_inside, false);
    }

    #[test]
    fn test_hit_test_point_complex() {
        let dwrite = DwriteFactory::new().unwrap();

        // Notes on this input:
        // 6 code points
        // 7 utf-16 code units (1/1/1/1/1/2)
        // 14 utf-8 code units (2/1/3/3/1/4)
        // 4 graphemes
        let input = "é\u{0023}\u{FE0F}\u{20E3}1\u{1D407}"; // #️⃣,, 𝐇

        let mut text_layout = D2DText::new(&dwrite);
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout.new_text_layout(&font, input).build().unwrap();
        println!("text pos 2: {:?}", layout.hit_test_text_position(2)); // 6.275390625
        println!("text pos 9: {:?}", layout.hit_test_text_position(9)); // 18.0
        println!("text pos 10: {:?}", layout.hit_test_text_position(10)); // 24.46875
        println!("text pos 14: {:?}", layout.hit_test_text_position(14)); // 33.3046875, line width

        let pt = layout.hit_test_point(Point::new(2.0, 0.0));
        assert_eq!(pt.metrics.text_position, 0);
        let pt = layout.hit_test_point(Point::new(4.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
        let pt = layout.hit_test_point(Point::new(7.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
        let pt = layout.hit_test_point(Point::new(10.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
        let pt = layout.hit_test_point(Point::new(14.0, 0.0));
        assert_eq!(pt.metrics.text_position, 9);
        let pt = layout.hit_test_point(Point::new(18.0, 0.0));
        assert_eq!(pt.metrics.text_position, 9);
        let pt = layout.hit_test_point(Point::new(19.0, 0.0));
        assert_eq!(pt.metrics.text_position, 9);
        let pt = layout.hit_test_point(Point::new(23.0, 0.0));
        assert_eq!(pt.metrics.text_position, 10);
        let pt = layout.hit_test_point(Point::new(25.0, 0.0));
        assert_eq!(pt.metrics.text_position, 10);
        let pt = layout.hit_test_point(Point::new(32.0, 0.0));
        assert_eq!(pt.metrics.text_position, 14);
        let pt = layout.hit_test_point(Point::new(35.0, 0.0));
        assert_eq!(pt.metrics.text_position, 14);
    }

    #[test]
    fn test_count_until_utf16() {
        // Notes on this input:
        // 5 code points
        // 5 utf-16 code units
        // 10 utf-8 code units (2/1/3/3/1)
        // 3 graphemes
        let input = "é\u{0023}\u{FE0F}\u{20E3}1"; // #️⃣

        assert_eq!(count_until_utf16(input, 0), Some(0));
        assert_eq!(count_until_utf16(input, 1), Some(2));
        assert_eq!(count_until_utf16(input, 2), Some(3));
        assert_eq!(count_until_utf16(input, 3), Some(6));
        assert_eq!(count_until_utf16(input, 4), Some(9));
        assert_eq!(count_until_utf16(input, 5), None);
    }
}
