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
    let empty_layout = factory
        .new_text_layout("")
        .font(FontFamily::SYSTEM_UI, 24.0)
        .build()
        .unwrap();
    let non_empty_layout = factory
        .new_text_layout("-")
        .font(FontFamily::SYSTEM_UI, 24.0)
        .build()
        .unwrap();
    assert!(empty_layout.size().height > 0.0);
    assert_close!(
        empty_layout.size().height,
        non_empty_layout.size().height,
        2.0
    );
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
    let line_size = measure_width(&mut factory, "AA", FontFamily::SYSTEM_UI, 12.0);
    let layout = factory
        .new_text_layout(text)
        .max_width(line_size.width)
        .font(FontFamily::SYSTEM_UI, 12.0)
        .build()
        .unwrap();

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

fn measure_width(factory: &mut impl Text, text: &str, font: FontFamily, size: f64) -> Size {
    factory
        .new_text_layout(text.to_owned())
        .font(font, size)
        .build()
        .unwrap()
        .size()
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
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn trailing_whitespace_width() {
    let mut factory = make_factory();
    let text = "hello";
    let text_ws = "hello     ";
    let non_ws = factory.new_text_layout(text).build().unwrap();
    let ws = factory.new_text_layout(text_ws).build().unwrap();

    assert_close!(non_ws.size().width, ws.size().width, 0.1);
    assert_close!(non_ws.trailing_whitespace_width(), non_ws.size().width, 0.1);
    // the width with whitespace is ~very approximately~ twice the width without whitespace
    assert_close!(ws.trailing_whitespace_width() / ws.size().width, 2.0, 0.5);

    // https://github.com/linebender/piet/pull/407
    // check that we aren't miscalculating trailing whitespace width by (for instance)
    // incorrectly adding it to base width
    let text_ws_plus = "hello     +";
    let ws_plus = factory.new_text_layout(text_ws_plus).build().unwrap();
    assert!(
        ws_plus.trailing_whitespace_width() > ws.trailing_whitespace_width(),
        "trailing ws width is inclusive of other width"
    );
}
