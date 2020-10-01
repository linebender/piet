//! Basic conformance testing for text.

use kurbo::Point;
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
fn ws_only_layout() {
    let mut factory = make_factory();
    let text = " ";
    let layout = factory.new_text_layout(text).build().unwrap();
    assert_eq!(layout.line_count(), 1);
    assert!(layout.line_metric(0).is_some());
    assert!(layout.line_text(0).is_some());
}

#[test]
fn empty_layout_size() {
    let mut factory = make_factory();
    let empty_layout = factory.new_text_layout("").build().unwrap();
    let non_empty_layout = factory.new_text_layout("-").build().unwrap();
    assert!(empty_layout.size().height > 0.0);
    assert_close!(
        empty_layout.size().height,
        non_empty_layout.size().height,
        1.0
    );
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
