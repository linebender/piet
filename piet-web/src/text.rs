//! Text functionality for Piet web backend

mod grapheme;
mod lines;

use std::borrow::Cow;
use std::ops::RangeBounds;

use web_sys::CanvasRenderingContext2d;

use piet::kurbo::{Point, Rect, Size};

use piet::{
    Error, Font, FontBuilder, HitTestPoint, HitTestPosition, LineMetric, Text, TextAttribute,
    TextLayout, TextLayoutBuilder,
};
use unicode_segmentation::UnicodeSegmentation;

use self::grapheme::{get_grapheme_boundaries, point_x_in_grapheme};
use crate::WebText;

#[derive(Clone)]
pub struct WebFont {
    family: String,
    weight: u32,
    style: FontStyle,
    size: f64,
}

pub struct WebFontBuilder(WebFont);

#[derive(Clone)]
pub struct WebTextLayout {
    ctx: CanvasRenderingContext2d,
    // TODO like cairo, should this be pub(crate)?
    pub font: WebFont,
    pub text: String,

    // Calculated on build
    pub(crate) line_metrics: Vec<LineMetric>,
    size: Size,
}

pub struct WebTextLayoutBuilder {
    ctx: CanvasRenderingContext2d,
    font: WebFont,
    text: String,
    width: f64,
}

/// https://developer.mozilla.org/en-US/docs/Web/CSS/font-style
#[allow(dead_code)] // TODO: Remove
#[derive(Clone)]
enum FontStyle {
    Normal,
    Italic,
    Oblique(Option<f64>),
}

impl Text for WebText {
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

    fn system_font(&mut self, size: f64) -> Self::Font {
        let font = WebFont {
            family: "sans-serif".to_owned(),
            size,
            weight: 400,
            style: FontStyle::Normal,
        };
        WebFontBuilder(font).build().unwrap()
    }

    fn new_text_layout(&mut self, text: &str) -> Self::TextLayoutBuilder {
        WebTextLayoutBuilder {
            // TODO: it's very likely possible to do this without cloning ctx, but
            // I couldn't figure out the lifetime errors from a `&'a` reference.
            ctx: self.ctx.clone(),
            font: self.system_font(piet::util::DEFAULT_FONT_SIZE),
            text: text.to_owned(),
            width: f64::INFINITY,
        }
    }
}

impl FontBuilder for WebFontBuilder {
    type Out = WebFont;

    fn build(self) -> Result<Self::Out, Error> {
        Ok(self.0)
    }
}

impl Font for WebFont {}

impl WebFont {
    // TODO should this be pub(crate)?
    pub fn get_font_string(&self) -> String {
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
    type Font = WebFont;

    fn max_width(mut self, width: f64) -> Self {
        self.width = width;
        self
    }

    fn alignment(self, _alignment: piet::TextAlignment) -> Self {
        web_sys::console::log_1(&"TextLayout alignment unsupported on web".into());
        self
    }

    fn default_attribute(self, _attribute: impl Into<TextAttribute<Self::Font>>) -> Self {
        web_sys::console::log_1(&"Text attributes not yet implemented for web".into());
        self
    }

    fn range_attribute(
        self,
        _range: impl RangeBounds<usize>,
        _attribute: impl Into<TextAttribute<Self::Font>>,
    ) -> Self {
        web_sys::console::log_1(&"Text attributes not yet implemented for web".into());
        self
    }

    fn build(self) -> Result<Self::Out, Error> {
        self.ctx.set_font(&self.font.get_font_string());

        let mut layout = WebTextLayout {
            ctx: self.ctx,
            font: self.font,
            text: self.text,
            line_metrics: Vec::new(),
            size: Size::ZERO,
        };

        layout.update_width(self.width)?;
        Ok(layout)
    }
}

impl TextLayout for WebTextLayout {
    fn width(&self) -> f64 {
        // precalculated on textlayout build
        self.size.width
    }

    fn size(&self) -> Size {
        self.size
    }

    fn image_bounds(&self) -> Rect {
        //FIXME: figure out actual image bounds on web?
        self.size.to_rect()
    }

    fn update_width(&mut self, new_width: impl Into<Option<f64>>) -> Result<(), Error> {
        let new_width = new_width.into().unwrap_or(std::f64::INFINITY);

        let line_metrics =
            lines::calculate_line_metrics(&self.text, &self.ctx, new_width, self.font.size);

        let max_width = line_metrics
            .iter()
            .map(|lm| text_width(&self.text[lm.start_offset..lm.end_offset], &self.ctx))
            .fold(0., f64::max);
        let height = line_metrics
            .last()
            .map(|l| l.y_offset + l.height)
            .unwrap_or_default();
        self.line_metrics = line_metrics;
        self.size = Size::new(max_width, height);
        Ok(())
    }

    fn line_text(&self, line_number: usize) -> Option<&str> {
        self.line_metrics
            .get(line_number)
            .map(|lm| &self.text[lm.start_offset..lm.end_offset])
    }

    fn line_metric(&self, line_number: usize) -> Option<LineMetric> {
        self.line_metrics.get(line_number).cloned()
    }

    fn line_count(&self) -> usize {
        self.line_metrics.len()
    }

    fn hit_test_point(&self, point: Point) -> HitTestPoint {
        // internal logic is using grapheme clusters, but return the text position associated
        // with the border of the grapheme cluster.

        // null case
        if self.text.is_empty() {
            return HitTestPoint::default();
        }

        // this assumes that all heights/baselines are the same.
        // Uses line bounding box to do hit testpoint, but with coordinates starting at 0.0 at
        // first baseline
        let first_baseline = self.line_metrics.get(0).map(|l| l.baseline).unwrap_or(0.0);

        // check out of bounds above top
        // out of bounds on bottom during iteration
        let mut is_y_inside = true;
        if point.y < -1.0 * first_baseline {
            is_y_inside = false
        };

        let mut lm = self
            .line_metrics
            .iter()
            .skip_while(|l| l.y_offset + l.height < point.y);
        let lm = lm
            .next()
            .or_else(|| {
                // This means it went over the last line, so return the last line.
                is_y_inside = false;
                self.line_metrics.last()
            })
            .cloned() // TODO remove this clone?
            .unwrap_or_else(|| {
                is_y_inside = false;
                Default::default()
            });

        // Then for the line, do hit test point
        // Trailing whitespace is remove for the line
        let line = &self.text[lm.start_offset..lm.end_offset];

        let mut htp = hit_test_line_point(&self.ctx, line, point);
        htp.idx += lm.start_offset;

        if !is_y_inside {
            htp.is_inside = false;
        }

        htp
    }

    fn hit_test_text_position(&self, text_position: usize) -> Option<HitTestPosition> {
        // first need to find line it's on, and get line start offset
        let lm = self
            .line_metrics
            .iter()
            .take_while(|l| l.start_offset <= text_position)
            .last()
            .cloned()
            .unwrap_or_else(Default::default);

        let count = self
            .line_metrics
            .iter()
            .take_while(|l| l.start_offset <= text_position)
            .count();

        // In web toy text, all baselines and heights are the same.
        // We're counting the first line baseline as 0, and measuring to each line's baseline.
        let y = if count == 0 {
            return Some(HitTestPosition::default());
        } else {
            (count - 1) as f64 * lm.height
        };

        // Then for the line, do text position
        // Trailing whitespace is removed for the line
        let line = &self.text[lm.start_offset..lm.end_offset];
        let line_position = text_position - lm.start_offset;

        let mut http = hit_test_line_position(&self.ctx, line, line_position);
        if let Some(h) = http.as_mut() {
            h.point.y = y;
        };
        http
    }
}

// NOTE this is the same as the old, non-line-aware version of hit_test_point
// Future: instead of passing ctx, should there be some other line-level text layout?
fn hit_test_line_point(ctx: &CanvasRenderingContext2d, text: &str, point: Point) -> HitTestPoint {
    // null case
    if text.is_empty() {
        return HitTestPoint::default();
    }

    // get bounds
    // TODO handle if string is not null yet count is 0?
    let end = UnicodeSegmentation::graphemes(text, true).count() - 1;
    let end_bounds = match get_grapheme_boundaries(ctx, text, end) {
        Some(bounds) => bounds,
        None => return HitTestPoint::default(),
    };

    let start = 0;
    let start_bounds = match get_grapheme_boundaries(ctx, text, start) {
        Some(bounds) => bounds,
        None => return HitTestPoint::default(),
    };

    // first test beyond ends
    if point.x > end_bounds.trailing {
        return HitTestPoint {
            idx: text.len(),
            is_inside: false,
        };
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

        let grapheme_bounds = match get_grapheme_boundaries(ctx, text, middle) {
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

// NOTE this is the same as the old, non-line-aware version of hit_test_text_position.
// Future: instead of passing ctx, should there be some other line-level text layout?
fn hit_test_line_position(
    ctx: &CanvasRenderingContext2d,
    text: &str,
    text_position: usize,
) -> Option<HitTestPosition> {
    // Using substrings with unicode grapheme awareness

    let text_len = text.len();

    if text_position == 0 {
        return Some(HitTestPosition::default());
    }

    if text_position as usize >= text_len {
        return Some(HitTestPosition {
            point: Point {
                x: text_width(text, ctx),
                y: 0.0,
            },
        });
    }

    // Already checked that text_position > 0 and text_position < count.
    // If text position is not at a grapheme boundary, use the text position of current
    // grapheme cluster. But return the original text position
    // Use the indices (byte offset, which for our purposes = utf8 code units).
    let grapheme_indices = UnicodeSegmentation::grapheme_indices(text, true)
        .take_while(|(byte_idx, _s)| text_position >= *byte_idx);

    if let Some((byte_idx, _s)) = grapheme_indices.last() {
        let point_x = text_width(&text[0..byte_idx], ctx);

        Some(HitTestPosition {
            point: Point::new(point_x, 0.),
        })
    } else {
        // iterated to end boundary
        Some(HitTestPosition {
            point: Point::new(text_width(text, ctx), 0.0),
        })
    }
}

pub(crate) fn text_width(text: &str, ctx: &CanvasRenderingContext2d) -> f64 {
    ctx.measure_text(text)
        .map(|m| m.width())
        .expect("Text measurement failed")
}

// NOTE these tests are currently only working on chrome.
// Since it's so finicky, not sure it's worth making it work on both chrome and firefox until we
// address the underlying brittlness
#[cfg(test)]
pub(crate) mod test {
    use piet::kurbo::Point;
    use piet::{FontBuilder, Text, TextLayout, TextLayoutBuilder};
    use wasm_bindgen_test::*;
    use web_sys::{console, window, HtmlCanvasElement};

    use crate::*;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    fn setup_ctx() -> (Window, CanvasRenderingContext2d) {
        let window = window().unwrap();
        let document = window.document().unwrap();

        let canvas = document
            .create_element("canvas")
            .unwrap()
            .dyn_into::<HtmlCanvasElement>()
            .unwrap();
        let context = canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .unwrap();

        let dpr = window.device_pixel_ratio();
        canvas.set_width((canvas.offset_width() as f64 * dpr) as u32);
        canvas.set_height((canvas.offset_height() as f64 * dpr) as u32);
        let _ = context.scale(dpr, dpr);

        (window, context)
    }

    // - x: calculated value
    // - target: f64
    // - tolerance: in f64
    fn assert_close_to(x: f64, target: f64, tolerance: f64) {
        let min = target - tolerance;
        let max = target + tolerance;
        println!("x: {}, target: {}", x, target);
        assert!(x <= max && x >= min);
    }

    #[wasm_bindgen_test]
    pub fn test_hit_test_text_position_basic() {
        let (_window, context) = setup_ctx();
        let mut text_layout = WebText::new(context);

        let input = "piet text!";
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();

        let layout = text_layout
            .new_text_layout(&input[0..4])
            .font(font.clone(), 12.0)
            .build()
            .unwrap();
        let piet_width = layout.size().width;

        let layout = text_layout
            .new_text_layout(&input[0..3])
            .font(font.clone(), 12.0)
            .build()
            .unwrap();
        let pie_width = layout.size().width;

        let layout = text_layout
            .new_text_layout(&input[0..2])
            .font(font.clone(), 12.0)
            .build()
            .unwrap();
        let pi_width = layout.size().width;

        let layout = text_layout
            .new_text_layout(&input[0..1])
            .font(font.clone(), 12.0)
            .build()
            .unwrap();
        let p_width = layout.size().width;

        let layout = text_layout
            .new_text_layout("")
            .font(font.clone(), 12.0)
            .build()
            .unwrap();
        let null_width = layout.size().width;

        let full_layout = text_layout
            .new_text_layout(input)
            .font(font, 12.0)
            .build()
            .unwrap();
        let full_width = full_layout.size().width;

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
        assert_close_to(
            full_layout.hit_test_text_position(11).unwrap().point.x as f64,
            full_width,
            3.0,
        );
    }

    #[wasm_bindgen_test]
    pub fn test_hit_test_text_position_complex_0() {
        let (_window, context) = setup_ctx();
        let mut text_layout = WebText::new(context);

        let input = "é";
        assert_eq!(input.len(), 2);

        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout
            .new_text_layout(input)
            .font(font, 12.0)
            .build()
            .unwrap();

        assert_close_to(layout.hit_test_text_position(0).unwrap().point.x, 0.0, 3.0);
        assert_close_to(
            layout.hit_test_text_position(2).unwrap().point.x,
            layout.size().width,
            3.0,
        );

        // note code unit not at grapheme boundary
        // This one panics in d2d because this is not a code unit boundary.
        // But it works here! Harder to deal with this right now, since unicode-segmentation
        // doesn't give code point offsets.
        assert_close_to(layout.hit_test_text_position(1).unwrap().point.x, 0.0, 3.0);

        // unicode segmentation is wrong on this one for now.
        //let input = "🤦\u{1f3fc}\u{200d}\u{2642}\u{fe0f}";

        //let mut text_layout = D2DText::new();
        //let font = text_layout.new_font_by_name("sans-serif", 12.0).build().unwrap();
        //let layout = text_layout.new_text_layout(&font, input, std::f64::INFINITY).build().unwrap();

        //assert_eq!(input.graphemes(true).count(), 1);
        //assert_eq!(layout.hit_test_text_position(0, true).map(|p| p.point_x as f64), Some(layout.size().width));
        //assert_eq!(input.len(), 17);

        let input = "\u{0023}\u{FE0F}\u{20E3}"; // #️⃣
        assert_eq!(input.len(), 7);
        assert_eq!(input.chars().count(), 3);

        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout
            .new_text_layout(input)
            .font(font, 12.0)
            .build()
            .unwrap();

        assert_close_to(layout.hit_test_text_position(0).unwrap().point.x, 0.0, 3.0);
        assert_close_to(
            layout.hit_test_text_position(7).unwrap().point.x,
            layout.size().width,
            3.0,
        );

        // note code unit not at grapheme boundary
        assert_close_to(layout.hit_test_text_position(1).unwrap().point.x, 0.0, 3.0);
    }

    #[wasm_bindgen_test]
    pub fn test_hit_test_text_position_complex_1() {
        let (_window, context) = setup_ctx();
        let mut text_layout = WebText::new(context);

        // Notes on this input:
        // 6 code points
        // 7 utf-16 code units (1/1/1/1/1/2)
        // 14 utf-8 code units (2/1/3/3/1/4)
        // 4 graphemes
        let input = "é\u{0023}\u{FE0F}\u{20E3}1\u{1D407}"; // #️⃣,, 𝐇
        assert_eq!(input.len(), 14);

        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout
            .new_text_layout(input)
            .font(font.clone(), 12.0)
            .build()
            .unwrap();

        let test_layout_0 = text_layout
            .new_text_layout(&input[0..2])
            .font(font.clone(), 12.0)
            .build()
            .unwrap();
        let test_layout_1 = text_layout
            .new_text_layout(&input[0..9])
            .font(font.clone(), 12.0)
            .build()
            .unwrap();
        let test_layout_2 = text_layout
            .new_text_layout(&input[0..10])
            .font(font, 12.0)
            .build()
            .unwrap();

        // Note: text position is in terms of utf8 code units
        assert_close_to(layout.hit_test_text_position(0).unwrap().point.x, 0.0, 3.0);
        assert_close_to(
            layout.hit_test_text_position(2).unwrap().point.x,
            test_layout_0.size().width,
            3.0,
        );
        assert_close_to(
            layout.hit_test_text_position(9).unwrap().point.x,
            test_layout_1.size().width,
            3.0,
        );
        assert_close_to(
            layout.hit_test_text_position(10).unwrap().point.x,
            test_layout_2.size().width,
            3.0,
        );
        assert_close_to(
            layout.hit_test_text_position(14).unwrap().point.x,
            layout.size().width,
            3.0,
        );

        // Code point boundaries, but not grapheme boundaries.
        // Width should stay at the last complete grapheme boundary.
        assert_close_to(layout.hit_test_text_position(1).unwrap().point.x, 0.0, 3.0);
        assert_close_to(
            layout.hit_test_text_position(3).unwrap().point.x,
            test_layout_0.size().width,
            3.0,
        );
        assert_close_to(
            layout.hit_test_text_position(6).unwrap().point.x,
            test_layout_0.size().width,
            3.0,
        );
    }

    // NOTE brittle test
    #[wasm_bindgen_test]
    pub fn test_hit_test_point_basic_0() {
        let (_window, context) = setup_ctx();
        let mut text_layout = WebText::new(context);

        let font = text_layout
            .new_font_by_name("sans-serif", 16.0)
            .build()
            .unwrap();
        let layout = text_layout
            .new_text_layout("piet text!")
            .font(font, 16.0)
            .build()
            .unwrap();
        console::log_1(&format!("text pos 4: {:?}", layout.hit_test_text_position(4)).into()); // 23.99...
        console::log_1(&format!("text pos 5: {:?}", layout.hit_test_text_position(5)).into()); // 27.99...

        // test hit test point
        // all inside
        let pt = layout.hit_test_point(Point::new(22.5, 0.0));
        assert_eq!(pt.idx, 4);
        let pt = layout.hit_test_point(Point::new(23.0, 0.0));
        assert_eq!(pt.idx, 4);
        let pt = layout.hit_test_point(Point::new(25.0, 0.0));
        assert_eq!(pt.idx, 4);
        let pt = layout.hit_test_point(Point::new(26.0, 0.0));
        assert_eq!(pt.idx, 5);
        let pt = layout.hit_test_point(Point::new(27.0, 0.0));
        assert_eq!(pt.idx, 5);
        let pt = layout.hit_test_point(Point::new(28.0, 0.0));
        assert_eq!(pt.idx, 5);

        // outside
        console::log_1(&format!("layout_width: {:?}", layout.size().width).into()); // 57.31...

        let pt = layout.hit_test_point(Point::new(55.0, 0.0));
        assert_eq!(pt.idx, 10); // last text position
        assert_eq!(pt.is_inside, true);

        let pt = layout.hit_test_point(Point::new(58.0, 0.0));
        assert_eq!(pt.idx, 10); // last text position
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(-1.0, 0.0));
        assert_eq!(pt.idx, 0); // first text position
        assert_eq!(pt.is_inside, false);
    }

    // NOTE brittle test
    #[wasm_bindgen_test]
    pub fn test_hit_test_point_basic_1() {
        let (_window, context) = setup_ctx();
        let mut text_layout = WebText::new(context);

        // base condition, one grapheme
        let font = text_layout
            .new_font_by_name("sans-serif", 16.0)
            .build()
            .unwrap();
        let layout = text_layout
            .new_text_layout("t")
            .font(font.clone(), 16.0)
            .build()
            .unwrap();
        println!("text pos 1: {:?}", layout.hit_test_text_position(1)); // 5.0

        // two graphemes (to check that middle moves)
        let pt = layout.hit_test_point(Point::new(1.0, 0.0));
        assert_eq!(pt.idx, 0);

        let layout = text_layout
            .new_text_layout("te")
            .font(font, 16.0)
            .build()
            .unwrap();
        println!("text pos 1: {:?}", layout.hit_test_text_position(1)); // 5.0
        println!("text pos 2: {:?}", layout.hit_test_text_position(2)); // 12.0

        let pt = layout.hit_test_point(Point::new(1.0, 0.0));
        assert_eq!(pt.idx, 0);
        let pt = layout.hit_test_point(Point::new(4.0, 0.0));
        assert_eq!(pt.idx, 1);
        let pt = layout.hit_test_point(Point::new(6.0, 0.0));
        assert_eq!(pt.idx, 1);
        let pt = layout.hit_test_point(Point::new(11.0, 0.0));
        assert_eq!(pt.idx, 2);
    }

    // NOTE brittle test
    #[wasm_bindgen_test]
    pub fn test_hit_test_point_complex_0() {
        let (_window, context) = setup_ctx();
        let mut text_layout = WebText::new(context);

        // Notes on this input:
        // 6 code points
        // 7 utf-16 code units (1/1/1/1/1/2)
        // 14 utf-8 code units (2/1/3/3/1/4)
        // 4 graphemes
        let input = "é\u{0023}\u{FE0F}\u{20E3}1\u{1D407}"; // #️⃣,, 𝐇

        let font = text_layout
            .new_font_by_name("sans-serif", 13.0) // font size hacked to fit test
            .build()
            .unwrap();
        let layout = text_layout
            .new_text_layout(input)
            .font(font, 13.0)
            .build()
            .unwrap();
        console::log_1(&format!("text pos 2: {:?}", layout.hit_test_text_position(2)).into()); // 5.77...
        console::log_1(&format!("text pos 9: {:?}", layout.hit_test_text_position(9)).into()); // 21.77...
        console::log_1(&format!("text pos 10: {:?}", layout.hit_test_text_position(10)).into()); // 28.27...
        console::log_1(&format!("text pos 14: {:?}", layout.hit_test_text_position(14)).into()); // 38.27..., line width

        let pt = layout.hit_test_point(Point::new(2.0, 0.0));
        assert_eq!(pt.idx, 0);
        let pt = layout.hit_test_point(Point::new(4.0, 0.0));
        assert_eq!(pt.idx, 2);
        let pt = layout.hit_test_point(Point::new(7.0, 0.0));
        assert_eq!(pt.idx, 2);
        let pt = layout.hit_test_point(Point::new(10.0, 0.0));
        assert_eq!(pt.idx, 2);
        let pt = layout.hit_test_point(Point::new(14.0, 0.0));
        assert_eq!(pt.idx, 9);
        let pt = layout.hit_test_point(Point::new(18.0, 0.0));
        assert_eq!(pt.idx, 9);
        let pt = layout.hit_test_point(Point::new(23.0, 0.0));
        assert_eq!(pt.idx, 9);
        let pt = layout.hit_test_point(Point::new(26.0, 0.0));
        assert_eq!(pt.idx, 10);
        let pt = layout.hit_test_point(Point::new(29.0, 0.0));
        assert_eq!(pt.idx, 10);
        let pt = layout.hit_test_point(Point::new(32.0, 0.0));
        assert_eq!(pt.idx, 10);
        let pt = layout.hit_test_point(Point::new(35.5, 0.0));
        assert_eq!(pt.idx, 14);
        let pt = layout.hit_test_point(Point::new(38.0, 0.0));
        assert_eq!(pt.idx, 14);
        let pt = layout.hit_test_point(Point::new(40.0, 0.0));
        assert_eq!(pt.idx, 14);
    }

    // NOTE brittle test
    #[wasm_bindgen_test]
    pub fn test_hit_test_point_complex_1() {
        let (_window, context) = setup_ctx();
        let mut text_layout = WebText::new(context);

        // this input caused an infinite loop in the binary search when test position
        // > 21.0 && < 28.0
        //
        // This corresponds to the char 'y' in the input.
        let input = "tßßypi";

        let font = text_layout
            .new_font_by_name("sans-serif", 14.0)
            .build()
            .unwrap();
        let layout = text_layout
            .new_text_layout(input)
            .font(font, 14.0)
            .build()
            .unwrap();
        console::log_1(&format!("text pos 0: {:?}", layout.hit_test_text_position(0)).into()); // 0.0
        console::log_1(&format!("text pos 1: {:?}", layout.hit_test_text_position(1)).into()); // 3.88...
        console::log_1(&format!("text pos 2: {:?}", layout.hit_test_text_position(2)).into()); // 3.88...
        console::log_1(&format!("text pos 3: {:?}", layout.hit_test_text_position(3)).into()); // 10.88...
        console::log_1(&format!("text pos 4: {:?}", layout.hit_test_text_position(4)).into()); // 10.88...
        console::log_1(&format!("text pos 5: {:?}", layout.hit_test_text_position(5)).into()); // 17.88...
        console::log_1(&format!("text pos 6: {:?}", layout.hit_test_text_position(6)).into()); // 24.88...
        console::log_1(&format!("text pos 7: {:?}", layout.hit_test_text_position(7)).into()); // 31.88...
        console::log_1(&format!("text pos 8: {:?}", layout.hit_test_text_position(8)).into()); // 35.77..., end

        let pt = layout.hit_test_point(Point::new(27.0, 0.0));
        assert_eq!(pt.idx, 6);
    }

    #[wasm_bindgen_test]
    fn test_multiline_hit_test_text_position_basic() {
        let (_window, context) = setup_ctx();
        let mut text_layout = WebText::new(context);

        let input = "piet  text!";
        let font = text_layout
            .new_font_by_name("sans-serif", 15.0) // change this for osx
            .build()
            .unwrap();

        let layout = text_layout
            .new_text_layout(&input[0..3])
            .font(font.clone(), 15.0)
            .max_width(30.0)
            .build()
            .unwrap();
        let pie_width = layout.size().width;

        let layout = text_layout
            .new_text_layout(&input[0..4])
            .font(font.clone(), 15.0)
            .max_width(25.0)
            .build()
            .unwrap();
        let piet_width = layout.size().width;

        let layout = text_layout
            .new_text_layout(&input[0..5])
            .font(font.clone(), 15.0)
            .max_width(30.0)
            .build()
            .unwrap();
        let piet_space_width = layout.size().width;

        // "text" should be on second line
        let layout = text_layout
            .new_text_layout(&input[6..10])
            .font(font.clone(), 15.0)
            .max_width(25.0)
            .build()
            .unwrap();
        let text_width = layout.size().width;

        let layout = text_layout
            .new_text_layout(&input[6..9])
            .font(font.clone(), 15.0)
            .max_width(25.0)
            .build()
            .unwrap();
        let tex_width = layout.size().width;

        let layout = text_layout
            .new_text_layout(&input[6..8])
            .font(font.clone(), 15.0)
            .max_width(25.0)
            .build()
            .unwrap();
        let te_width = layout.size().width;

        let layout = text_layout
            .new_text_layout(&input[6..7])
            .font(font.clone(), 15.0)
            .max_width(25.0)
            .build()
            .unwrap();
        let t_width = layout.size().width;

        let full_layout = text_layout
            .new_text_layout(input)
            .font(font, 15.0)
            .max_width(25.0)
            .build()
            .unwrap();

        println!("lm: {:#?}", full_layout.line_metrics);
        println!("layout width: {:#?}", full_layout.size().width);

        println!("'pie': {}", pie_width);
        println!("'piet': {}", piet_width);
        println!("'piet ': {}", piet_space_width);
        println!("'text': {}", text_width);
        println!("'tex': {}", tex_width);
        println!("'te': {}", te_width);
        println!("'t': {}", t_width);

        // NOTE these heights are representative of baseline-to-baseline measures
        let line_zero_baseline = 0.0;
        let line_one_baseline = full_layout.line_metric(1).unwrap().height;

        // these just test the x position of text positions on the second line
        assert_close_to(
            full_layout.hit_test_text_position(10).unwrap().point.x as f64,
            text_width,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(9).unwrap().point.x as f64,
            tex_width,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(8).unwrap().point.x as f64,
            te_width,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(7).unwrap().point.x as f64,
            t_width,
            3.0,
        );
        // This should be beginning of second line
        assert_close_to(
            full_layout.hit_test_text_position(6).unwrap().point.x as f64,
            0.0,
            3.0,
        );

        assert_close_to(
            full_layout.hit_test_text_position(3).unwrap().point.x as f64,
            pie_width,
            3.0,
        );

        // This tests that trailing whitespace is included in the first line width.
        assert_close_to(
            full_layout.hit_test_text_position(5).unwrap().point.x as f64,
            piet_space_width,
            3.0,
        );

        // These test y position of text positions on line 1 (0-index)
        assert_close_to(
            full_layout.hit_test_text_position(10).unwrap().point.y as f64,
            line_one_baseline,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(9).unwrap().point.y as f64,
            line_one_baseline,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(8).unwrap().point.y as f64,
            line_one_baseline,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(7).unwrap().point.y as f64,
            line_one_baseline,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(6).unwrap().point.y as f64,
            line_one_baseline,
            3.0,
        );

        // this tests y position of 0 line
        assert_close_to(
            full_layout.hit_test_text_position(5).unwrap().point.y as f64,
            line_zero_baseline,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(4).unwrap().point.y as f64,
            line_zero_baseline,
            3.0,
        );
    }

    // very basic testing that multiline works
    #[wasm_bindgen_test]
    fn test_multiline_hit_test_point_basic() {
        let input = "piet text most best";

        let (_window, context) = setup_ctx();
        let mut text = WebText::new(context);

        let font = text.new_font_by_name("sans-serif", 14.0).build().unwrap();
        // this should break into four lines
        // Had to shift font in order to break at 4 lines (larger font than cairo, wider lines)
        let layout = text
            .new_text_layout(input)
            .font(font.clone(), 14.0)
            .max_width(30.0)
            .build()
            .unwrap();
        console::log_1(&format!("text pos 01: {:?}", layout.hit_test_text_position(0)).into()); // (0.0,0.0)
        console::log_1(&format!("text pos 06: {:?}", layout.hit_test_text_position(5)).into()); // (0.0, 16.8)
        console::log_1(&format!("text pos 11: {:?}", layout.hit_test_text_position(10)).into()); // (0.0, 33.6)
        console::log_1(&format!("text pos 16: {:?}", layout.hit_test_text_position(15)).into()); // (0.0, 50.4)
        console::log_1(&format!("lm 0: {:?}", layout.line_metric(0)).into());
        console::log_1(&format!("lm 1: {:?}", layout.line_metric(1)).into());
        console::log_1(&format!("lm 2: {:?}", layout.line_metric(2)).into());
        console::log_1(&format!("lm 3: {:?}", layout.line_metric(3)).into());

        // approx 13.5 baseline, and 17 height
        let pt = layout.hit_test_point(Point::new(1.0, -1.0));
        assert_eq!(pt.idx, 0);
        assert_eq!(pt.is_inside, true);
        let pt = layout.hit_test_point(Point::new(1.0, 00.0));
        assert_eq!(pt.idx, 0);
        let pt = layout.hit_test_point(Point::new(1.0, 04.0));
        assert_eq!(pt.idx, 5);
        let pt = layout.hit_test_point(Point::new(1.0, 21.0));
        assert_eq!(pt.idx, 10);
        let pt = layout.hit_test_point(Point::new(1.0, 38.0));
        assert_eq!(pt.idx, 15);

        // over on y axis, but x still affects the text position
        let best_layout = text
            .new_text_layout("best")
            .font(font.clone(), 14.0)
            .build()
            .unwrap();
        console::log_1(&format!("layout width: {:#?}", best_layout.size().width).into()); // 22.55...

        let pt = layout.hit_test_point(Point::new(1.0, 55.0));
        assert_eq!(pt.idx, 15);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(25.0, 55.0));
        assert_eq!(pt.idx, 19);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(27.0, 55.0));
        assert_eq!(pt.idx, 19);
        assert_eq!(pt.is_inside, false);

        // under
        let piet_layout = text
            .new_text_layout("piet ")
            .font(font, 14.0)
            .build()
            .unwrap();
        console::log_1(&format!("layout width: {:#?}", piet_layout.size().width).into()); // 24.49...

        let pt = layout.hit_test_point(Point::new(1.0, -14.0)); // under
        assert_eq!(pt.idx, 0);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(25.0, -14.0)); // under
        assert_eq!(pt.idx, 5);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(27.0, -14.0)); // under
        assert_eq!(pt.idx, 5);
        assert_eq!(pt.is_inside, false);
    }
}
