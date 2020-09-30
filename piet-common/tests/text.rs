//! Basic conformance testing for text.

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
