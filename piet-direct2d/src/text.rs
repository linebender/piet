pub use d2d::{D2DDevice, D2DFactory, DeviceContext as D2DDeviceContext};
pub use dwrite::DwriteFactory;

use std::convert::TryInto;

use piet::kurbo::Point;

use piet::{
    Error, Font, FontBuilder, HitTestMetrics, HitTestPoint, HitTestTextPosition, Text, TextLayout,
    TextLayoutBuilder,
};

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

pub struct D2DTextLayout {
    pub text: String,
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
    use crate::*;

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
