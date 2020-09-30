//! Basic conformance testing for text.

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

/// Text with a newline at EOF should have one more line reported than
/// the same text without the newline.
#[test]
fn newline_eof() {
    let mut factory = make_factory();
    let text = "A";
    let layout = factory.new_text_layout(text).build().unwrap();
    assert_eq!(layout.line_count(), 1);

    let text = "\n";
    let layout = factory.new_text_layout(text).build().unwrap();
    assert_eq!(layout.line_count(), 2);

    let text = "AA";
    let layout = factory.new_text_layout(text).build().unwrap();
    assert_eq!(layout.line_count(), 1);

    let text = "AA\n";
    let layout_newline = factory.new_text_layout(text).build().unwrap();
    assert_eq!(layout_newline.line_count(), 2);
    // newline should be reported in size
    // 1.5 is an arbitrary factor; we should be roughly 2x taller.
    assert!(layout.size().height * 1.5 < layout_newline.size().height);
}

//NOTE I actually don't know what I expect here
#[test]
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
fn empty_layout_size() {
    let mut factory = make_factory();
    let empty_layout = factory.new_text_layout("").build().unwrap();
    let non_empty_layout = factory.new_text_layout("-").build().unwrap();
    assert!(empty_layout.size().height > 0.0);
    assert_close!(empty_layout.size().height, non_empty_layout.size().height, 1.0);
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
