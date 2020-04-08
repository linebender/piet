//! Text functionality for Piet direct2d backend

mod lines;

pub use d2d::{D2DDevice, D2DFactory, DeviceContext as D2DDeviceContext};
pub use dwrite::DwriteFactory;

use std::convert::TryInto;

use piet::kurbo::Point;

use piet::{
    Error, Font, FontBuilder, HitTestMetrics, HitTestPoint, HitTestTextPosition, LineMetric, Text,
    TextLayout, TextLayoutBuilder,
};

use self::lines::fetch_line_metrics;
use crate::d2d;
use crate::dwrite::{self, TextFormat, TextFormatBuilder};

pub struct D2DText<'a> {
    dwrite: &'a DwriteFactory,
}

pub struct D2DFont(TextFormat);

pub struct D2DFontBuilder<'a> {
    builder: TextFormatBuilder<'a>,
    name: String,
}

#[derive(Clone)]
pub struct D2DTextLayout {
    pub text: String,
    // currently calculated on build
    line_metrics: Vec<LineMetric>,
    pub layout: dwrite::TextLayout,
}

pub struct D2DTextLayoutBuilder<'a> {
    text: String,
    builder: dwrite::TextLayoutBuilder<'a>,
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

    fn new_text_layout(
        &mut self,
        font: &Self::Font,
        text: &str,
        width: f64,
    ) -> Self::TextLayoutBuilder {
        D2DTextLayoutBuilder {
            text: text.to_owned(),
            builder: dwrite::TextLayoutBuilder::new(self.dwrite)
                .format(&font.0)
                .width(width as f32)
                .height(1e6)
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
        let layout = self.builder.build()?;

        let line_metrics = fetch_line_metrics(&layout);

        Ok(D2DTextLayout {
            text: self.text,
            line_metrics,
            layout,
        })
    }
}

impl TextLayout for D2DTextLayout {
    fn width(&self) -> f64 {
        self.layout.get_metrics().widthIncludingTrailingWhitespace as f64
    }

    /// given a new max width, update width of text layout to fit within the max width
    // TODO add this doc to trait method? or is this windows specific?
    fn update_width(&mut self, new_width: f64) -> Result<(), Error> {
        self.layout.set_max_width(new_width)?;
        self.line_metrics = fetch_line_metrics(&self.layout);

        Ok(())
    }

    fn line_text(&self, line_number: usize) -> Option<&str> {
        self.line_metrics
            .get(line_number)
            .map(|lm| &self.text[lm.start_offset..(lm.end_offset - lm.trailing_whitespace)])
    }

    fn line_metric(&self, line_number: usize) -> Option<LineMetric> {
        self.line_metrics.get(line_number).cloned()
    }

    fn line_count(&self) -> usize {
        self.line_metrics.len()
    }

    fn hit_test_point(&self, point: Point) -> HitTestPoint {
        // Before hit testing, need to convert point.y to have 0.0 at upper left corner (dwrite
        // style) instead of at first line baseline.
        let first_baseline = self
            .line_metrics
            .get(0)
            .map(|lm| lm.baseline)
            .unwrap_or(0.0);
        let y = point.y + first_baseline;

        // lossy from f64 to f32, but shouldn't have too much impact
        let htp = self.layout.hit_test_point(point.x as f32, y as f32);

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
            count_until_utf16(&self.text, text_position_16).unwrap_or_else(|| self.text.len());

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

        let trailing = false;

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
    #[allow(clippy::explicit_counter_loop)]
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
    use super::*;

    // - x: calculated value
    // - target: f64
    // - tolerance: in f64
    fn assert_close_to(x: f64, target: f64, tolerance: f64) {
        let min = target - tolerance;
        let max = target + tolerance;
        println!("x: {}, target: {}", x, target);
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
            .new_text_layout(&font, &input[0..4], std::f64::INFINITY)
            .build()
            .unwrap();
        let piet_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, &input[0..3], std::f64::INFINITY)
            .build()
            .unwrap();
        let pie_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, &input[0..2], std::f64::INFINITY)
            .build()
            .unwrap();
        let pi_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, &input[0..1], std::f64::INFINITY)
            .build()
            .unwrap();
        let p_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, "", std::f64::INFINITY)
            .build()
            .unwrap();
        let null_width = layout.width();

        let full_layout = text_layout
            .new_text_layout(&font, input, std::f64::INFINITY)
            .build()
            .unwrap();
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
        let layout = text_layout
            .new_text_layout(&font, input, std::f64::INFINITY)
            .build()
            .unwrap();

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
        //let layout = text_layout.new_text_layout(&font, input, std::f64::INFINITY).build().unwrap();

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
        let layout = text_layout
            .new_text_layout(&font, input, std::f64::INFINITY)
            .build()
            .unwrap();

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
        let layout = text_layout
            .new_text_layout(&font, input, std::f64::INFINITY)
            .build()
            .unwrap();

        let test_layout_0 = text_layout
            .new_text_layout(&font, &input[0..2], std::f64::INFINITY)
            .build()
            .unwrap();
        let test_layout_1 = text_layout
            .new_text_layout(&font, &input[0..9], std::f64::INFINITY)
            .build()
            .unwrap();
        let test_layout_2 = text_layout
            .new_text_layout(&font, &input[0..10], std::f64::INFINITY)
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
            .new_text_layout(&font, "piet text!", std::f64::INFINITY)
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
        let layout = text_layout
            .new_text_layout(&font, input, std::f64::INFINITY)
            .build()
            .unwrap();
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
    fn test_basic_multiline() {
        let input = "piet text most best";
        let width_small = 30.0;

        let dwrite = dwrite::DwriteFactory::new().unwrap();
        let mut text_layout = D2DText::new(&dwrite);
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout
            .new_text_layout(&font, input, width_small)
            .build()
            .unwrap();

        assert_eq!(layout.line_count(), 4);
        assert_eq!(layout.line_text(0), Some("piet"));
        assert_eq!(layout.line_text(1), Some("text"));
        assert_eq!(layout.line_text(2), Some("most"));
        assert_eq!(layout.line_text(3), Some("best"));
        assert_eq!(layout.line_text(4), None);
    }

    #[test]
    fn test_change_width() {
        let input = "piet text most best";
        let width_small = 30.0;
        let width_medium = 60.0;
        let width_large = 1000.0;

        let dwrite = dwrite::DwriteFactory::new().unwrap();
        let mut text_layout = D2DText::new(&dwrite);
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let mut layout = text_layout
            .new_text_layout(&font, input, width_small)
            .build()
            .unwrap();

        assert_eq!(layout.line_count(), 4);
        assert_eq!(layout.line_text(0), Some("piet"));

        layout.update_width(width_medium).unwrap();
        assert_eq!(layout.line_count(), 2);
        assert_eq!(layout.line_text(0), Some("piet text"));

        layout.update_width(width_large).unwrap();
        assert_eq!(layout.line_count(), 1);
        assert_eq!(layout.line_text(0), Some("piet text most best"));
    }

    // NOTE be careful, windows will break lines at the sub-word level!
    #[test]
    fn test_multiline_hit_test_text_position_basic() {
        let dwrite = dwrite::DwriteFactory::new().unwrap();
        let mut text_layout = D2DText::new(&dwrite);

        let input = "piet  text!";
        let font = text_layout
            .new_font_by_name("sans-serif", 15.0)
            .build()
            .unwrap();

        let layout = text_layout
            .new_text_layout(&font, &input[0..4], 30.0)
            .build()
            .unwrap();
        let piet_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, &input[0..3], 30.0)
            .build()
            .unwrap();
        let pie_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, &input[0..5], 30.0)
            .build()
            .unwrap();
        let piet_space_width = layout.width();

        // "text" should be on second line
        let layout = text_layout
            .new_text_layout(&font, &input[6..10], 30.0)
            .build()
            .unwrap();
        let text_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, &input[6..9], 30.0)
            .build()
            .unwrap();
        let tex_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, &input[6..8], 30.0)
            .build()
            .unwrap();
        let te_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, &input[6..7], 30.0)
            .build()
            .unwrap();
        let t_width = layout.width();

        let full_layout = text_layout
            .new_text_layout(&font, input, 30.0)
            .build()
            .unwrap();
        println!("lm: {:#?}", full_layout.line_metrics);
        println!("layout width: {:#?}", full_layout.width());

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

        // This tests that trailing whitespace is (or is not?) included in the first line width.
        // hit testing gives back something close to the full width (not line width) of text
        // layout.
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

    #[test]
    // very basic testing that multiline works
    fn test_multiline_hit_test_point_basic() {
        let input = "piet text most best";

        let dwrite = dwrite::DwriteFactory::new().unwrap();
        let mut text = D2DText::new(&dwrite);

        let font = text.new_font_by_name("sans-serif", 12.0).build().unwrap();
        // this should break into four lines
        let layout = text.new_text_layout(&font, input, 30.0).build().unwrap();
        println!("{}", layout.line_metric(0).unwrap().baseline); // 12.94...
        println!("text pos 01: {:?}", layout.hit_test_text_position(00)); // (0.0, 0.0)
        println!("text pos 06: {:?}", layout.hit_test_text_position(05)); // (0.0, 15.96...)
        println!("text pos 11: {:?}", layout.hit_test_text_position(10)); // (0.0, 31.92...)
        println!("text pos 16: {:?}", layout.hit_test_text_position(15)); // (0.0, 47.88...)

        let pt = layout.hit_test_point(Point::new(1.0, -13.0)); // under
        assert_eq!(pt.metrics.text_position, 0);
        assert_eq!(pt.is_inside, false);
        let pt = layout.hit_test_point(Point::new(1.0, -1.0));
        println!("{:?}", pt);
        assert_eq!(pt.metrics.text_position, 0);
        assert_eq!(pt.is_inside, true);
        let pt = layout.hit_test_point(Point::new(1.0, 00.0));
        assert_eq!(pt.metrics.text_position, 0);
        let pt = layout.hit_test_point(Point::new(1.0, 04.0));
        assert_eq!(pt.metrics.text_position, 5);
        let pt = layout.hit_test_point(Point::new(1.0, 20.0));
        assert_eq!(pt.metrics.text_position, 10);
        let pt = layout.hit_test_point(Point::new(1.0, 36.0));
        assert_eq!(pt.metrics.text_position, 15);

        // over on y axis, but x still affects the text position
        let best_layout = text
            .new_text_layout(&font, "best", std::f64::INFINITY)
            .build()
            .unwrap();
        println!("layout width: {:#?}", best_layout.width()); // 22.48...

        let pt = layout.hit_test_point(Point::new(1.0, 52.0));
        assert_eq!(pt.metrics.text_position, 15);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(22.0, 52.0));
        assert_eq!(pt.metrics.text_position, 19);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(24.0, 52.0));
        assert_eq!(pt.metrics.text_position, 19);
        assert_eq!(pt.is_inside, false);
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
