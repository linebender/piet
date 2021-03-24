//! Basic conformance testing for text.

#[cfg(target_arch = "wasm32")]
extern crate wasm_bindgen_test;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::*;

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test_configure!(run_in_browser);

use kurbo::{Point, Size};
use piet_common::*;

macro_rules! assert_close {
    ($val:expr, $target:expr, $tolerance:expr) => {{
        let min = $target - $tolerance;
        let max = $target + $tolerance;
        if $val < min || $val > max {
            panic!(
                "value {} outside target {} with tolerance {}",
                $val, $target, $tolerance
            );
        }
    }};

    ($val:expr, $target:expr, $tolerance:expr,) => {{
        assert_close!($val, $target, $tolerance)
    }};
}

fn make_factory() -> PietText {
    let mut device = Device::new().unwrap();
    let mut target = device.bitmap_target(400, 400, 2.0).unwrap();
    let mut ctx = target.render_context();
    let text = ctx.text().to_owned();
    let _ = ctx.finish();
    text
}

/// An extension trait to make simple tasks less verbose
trait FactoryHelpers {
    type Layout: TextLayout;
    fn make_layout(
        &mut self,
        text: &str,
        font: FontFamily,
        size: f64,
        width: impl Into<Option<f64>>,
    ) -> Self::Layout;

    fn make_mono_12pt(&mut self, text: &str) -> Self::Layout {
        self.make_layout(text, FontFamily::MONOSPACE, 12.0, None)
    }

    fn measure_width(&mut self, text: &str, font: FontFamily, size: f64) -> Size {
        self.make_layout(text, font, size, None).size()
    }

    fn get_mono_width(&mut self, size: f64) -> f64 {
        //FIXME: would be nice to have this check against another glyph to ensure
        //we're actually monospace, but that would currently break cairo
        self.measure_width("a", FontFamily::MONOSPACE, size).width
    }
}

impl<T: Text> FactoryHelpers for T {
    type Layout = T::TextLayout;
    fn make_layout(
        &mut self,
        text: &str,
        font: FontFamily,
        size: f64,
        width: impl Into<Option<f64>>,
    ) -> Self::Layout {
        self.new_text_layout(text.to_owned())
            .font(font, size)
            .max_width(width.into().unwrap_or(f64::INFINITY))
            .build()
            .unwrap()
    }
}

// https://github.com/linebender/piet/issues/334
#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn negative_width_doesnt_crash() {
    let mut factory = make_factory();
    let text = "oops";
    let layout = factory.new_text_layout(text).max_width(-4.0).build();
    assert!(layout.is_ok())
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn empty_layout() {
    let mut factory = make_factory();
    let text = "";
    let layout = factory.new_text_layout(text).build().unwrap();
    // this has one reported line
    assert_eq!(layout.line_count(), 1);
    // you can get line metrics for the first line
    assert!(layout.line_metric(0).is_some());
    // and the text
    assert!(layout.line_text(0).is_some());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn ws_only_layout() {
    let mut factory = make_factory();
    let text = " ";
    let layout = factory.new_text_layout(text).build().unwrap();
    assert_eq!(layout.line_count(), 1);
    assert!(layout.line_metric(0).is_some());
    assert!(layout.line_text(0).is_some());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn rects_for_empty_range() {
    let mut factory = make_factory();
    let text = "";
    let layout = factory.new_text_layout(text).build().unwrap();
    assert!(layout.rects_for_range(0..0).is_empty());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn empty_layout_size() {
    let mut factory = make_factory();
    let empty_layout = factory.make_layout("", FontFamily::SYSTEM_UI, 24.0, None);
    let non_empty_layout = factory.make_layout("-", FontFamily::SYSTEM_UI, 24.0, None);
    assert!(empty_layout.size().height > 0.0);
    assert_close!(
        empty_layout.size().height,
        non_empty_layout.size().height,
        1.0
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn hit_test_multibyte_grapheme_position() {
    let mut factory = make_factory();
    // 5 graphemes, 6 code points, 14 bytes
    // ("a", 1, 1), (£, 1, 2), (€, 1, 3), (💴, 1, 4), (#️⃣, 2, 4)
    let input = "a£€💴\u{0023}\u{FE0F}";
    assert_eq!(input.len(), 14);
    assert_eq!(input.chars().count(), 6);

    let layout = factory.new_text_layout(input).build().unwrap();

    let p0 = layout.hit_test_text_position(0).point;
    let p1 = layout.hit_test_text_position(1).point;
    let p3 = layout.hit_test_text_position(3).point;
    let p6 = layout.hit_test_text_position(6).point;
    let p10 = layout.hit_test_text_position(10).point;
    // a codepoint that is not the start of a grapheme
    let p11 = layout.hit_test_text_position(11).point;
    let p14 = layout.hit_test_text_position(14).point;

    // just getting around float_cmp lint :shrug:
    assert_close!(p0.x, 0.0, 1e-6);
    assert!(p1.x > p0.x);
    assert!(p3.x > p1.x);
    assert!(p6.x > p3.x);
    assert!(p10.x > p6.x);

    // NOTE: this last case isn't well defined. We would like it to return
    // the location of the start of the grapheme, but coretext just
    // returns `0.0`.
    //
    // two codepoints in a single grapheme:
    //assert_eq!(p10.x, p11.x);

    // last text position should resolve to trailing edge
    assert!(p14.x > p11.x);
}

#[test]
#[should_panic(expected = "is_char_boundary")]
//FIXME: should_panic doesn't work on wasm? need to figure out an alternative.
//see https://github.com/rustwasm/wasm-bindgen/issues/2286
fn hit_test_interior_byte() {
    let mut factory = make_factory();
    // 5 graphemes, 6 code points, 14 bytes
    // ("a", 1, 1), (£, 1, 2), (€, 1, 3), (💴, 1, 4), (#️⃣, 2, 4)
    let input = "a£€💴\u{0023}\u{FE0F}";

    let layout = factory.new_text_layout(input).build().unwrap();
    let _ = layout.hit_test_text_position(7).point;
}

/// Text with a newline at EOF should have one more line reported than
/// the same text without the newline.
#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn newline_eof() {
    let mut factory = make_factory();
    let text = "A";
    let layout = factory.new_text_layout(text).build().unwrap();
    assert_eq!(layout.line_count(), 1);

    let text = "\n";
    let layout = factory.new_text_layout(text).build().unwrap();
    assert_eq!(layout.line_count(), 2);
    assert_eq!(layout.line_text(0), Some("\n"));
    assert_eq!(layout.line_text(1), Some(""));
    assert!(layout.line_text(2).is_none());

    let hit0 = layout.hit_test_text_position(0);
    let hit1 = layout.hit_test_text_position(1);
    assert_close!(hit0.point.y * 2., hit1.point.y, 5.0);
    assert_eq!(hit0.line, 0);
    assert_eq!(hit1.line, 1);

    let hit1 = layout.hit_test_point(Point::new(50., 50.));
    assert_eq!(hit1.idx, 1);

    //TODO: the right of the first line should result in an insert before the newline
    //let hit = layout.hit_test_point(Point::new(20., 1.));
    //assert_eq!(hit.idx, 0)

    let text = "AA";
    let layout = factory.new_text_layout(text).build().unwrap();
    assert_eq!(layout.line_count(), 1);

    let text = "AA\n";
    let layout_newline = factory.new_text_layout(text).build().unwrap();
    assert_eq!(layout_newline.line_count(), 2);

    // newline should be reported in size
    assert_close!(
        layout.size().height * 2.0,
        layout_newline.size().height,
        1.0
    );
}

#[test]
fn eol_hit_testing() {
    let mut factory = make_factory();

    let text = "AA AA\nAA";
    let line_size = factory.measure_width("AA", FontFamily::SYSTEM_UI, 12.0);
    let layout = factory.make_layout(text, FontFamily::SYSTEM_UI, 12.0, line_size.width);

    let metrics = layout.line_metric(0).unwrap();

    // first line: a soft break
    // we expect this to always give us the start of the next line, because
    // we don't handle affinity
    let right_of_line = layout.hit_test_point(Point::new(line_size.width + 3.0, 5.0));
    assert_eq!(right_of_line.idx, 3);
    let hit = layout.hit_test_text_position(right_of_line.idx);
    // left edge
    assert_close!(hit.point.x, 0.0, 1.0);
    // baseline of second line
    assert_close!(hit.point.y, metrics.height + metrics.baseline, 2.0);

    //second line: hard break
    //ideally this puts us on the second line, but before the newline?
    let right_of_line =
        layout.hit_test_point(Point::new(line_size.width + 3.0, metrics.height + 5.0));
    assert_eq!(right_of_line.idx, 5);
    let hit = layout.hit_test_text_position(right_of_line.idx);
    // right edge
    assert_close!(hit.point.x, line_size.width, 2.0);
    // baseline of second line
    assert_close!(hit.point.y, metrics.height + metrics.baseline, 2.0);
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn debug_impl_exists() {
    let mut factory = make_factory();
    let text = "";
    let layout_builder = factory.new_text_layout(text);
    let layout = factory.new_text_layout(text).build().unwrap();
    let _args = format_args!("{:?} {:?} {:?}", text, layout_builder, layout);
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn width_sanity() {
    let mut factory = make_factory();
    let text = "hello";
    let ws = factory.new_text_layout(text).build().unwrap();
    assert_eq!(ws.line_count(), 1);
    let lm = ws.line_metric(0).unwrap();
    assert_eq!(lm.start_offset, 0);
    assert_eq!(lm.end_offset, text.len());

    let width = ws.size().width;
    assert_close!(width, 27.0, 5.0);
}

#[test]
//FIXME: disabled on linux until pango lands, and wasm until we have proper text there.
//#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg(not(target_os = "linux"))]
fn emergency_break_selections() {
    let mut factory = make_factory();
    let mono_width = factory.get_mono_width(16.0);

    let text = std::iter::repeat('a').take(20).collect::<String>();
    let layout_width = mono_width * 6.5;
    let layout = factory.make_layout(&text, FontFamily::MONOSPACE, 16.0, layout_width);
    assert_eq!(layout.line_count(), 4);

    let rects = layout.rects_for_range(..);
    assert_eq!(rects.len(), 4);
    let second_line = rects[1];
    assert_close!(second_line.min_x(), 0.0, 1.0);
    assert_close!(second_line.max_x(), 6.0 * mono_width, 1.0);
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn trailing_whitespace_width() {
    let mut factory = make_factory();
    let text = "hello";
    let text_ws = "hello     ";
    //let non_ws = make_layout(&mut factory, text, FontFamily::MONOSPACE, 12.0);
    let non_ws = factory.make_mono_12pt(text);
    let ws = factory.make_mono_12pt(text_ws);

    assert_close!(non_ws.size().width, ws.size().width, 0.1);
    assert_close!(non_ws.trailing_whitespace_width(), non_ws.size().width, 0.1);
    // the width with whitespace is ~approximately~ twice the width without
    assert_close!(ws.trailing_whitespace_width() / ws.size().width, 2.0, 0.1);
    // https://github.com/linebender/piet/pull/407
    // check that we aren't miscalculating trailing whitespace width by
    // (for instance) incorrectly adding it to base width
    let text_ws_plus = "hello     +";
    let ws_plus = factory.make_mono_12pt(text_ws_plus);
    assert!(
        ws_plus.trailing_whitespace_width() > ws.trailing_whitespace_width(),
        "trailing ws width is inclusive of other width"
    );
}
