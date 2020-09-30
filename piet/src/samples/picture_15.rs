//! Visualize results of hit-testing

use crate::kurbo::{Line, Size, Vec2};
use crate::{Color, Error, RenderContext, Text, TextAttribute, TextLayout, TextLayoutBuilder};

pub const SIZE: Size = Size::new(480., 480.);

static TEXT: &str = "AAA BBB  AAA\n";
const FONT_SIZE: f64 = 40.0;

const LIGHT_GREY: Color = Color::grey8(0xc0);
const RED: Color = Color::rgba8(255, 0, 0, 100);
const BLUE: Color = Color::rgba8(0, 0, 255, 100);

pub fn draw<R: RenderContext>(rc: &mut R) -> Result<(), Error> {
    rc.clear(LIGHT_GREY);
    let wrap_width = calculate_wrap_width(rc.text());
    let layout = rc.text()
        .new_text_layout(TEXT)
        .default_attribute(TextAttribute::FontSize(FONT_SIZE))
        .max_width(wrap_width)
        .build()?;

    let y_pos = ((SIZE.height - layout.size().height * 2.0) / 4.0).max(0.0);

    let text_pos = Vec2::new(16.0, y_pos);
    let layout_rect = layout.size().to_rect() + text_pos;
    rc.fill(layout_rect, &Color::WHITE);

    for idx in 0..=TEXT.len() {
        let color = if idx % 2 == 0 { &RED } else { &BLUE };
        let line = line_for_idx(idx, &layout) + text_pos;
        rc.stroke(line, color, 2.0);
        let label = text_label_for_idx(idx, rc.text());
        rc.draw_text(&label, line.p0);
    }
    rc.draw_text(&layout, text_pos.to_point());

    let space_cursor = line_for_idx(4, &layout) + text_pos;
    rc.stroke(space_cursor, &Color::BLACK, 1.0);

    Ok(())
}

/// Different fonts (on different platforms) will have different metrics;
/// we want to ensure we choose a wrap width that always wraps at the first space.
fn calculate_wrap_width(text: &mut impl Text) -> f64 {
    let first_line_no_ws = TEXT.split_whitespace().next().unwrap();
    text.new_text_layout(first_line_no_ws)
        .default_attribute(TextAttribute::FontSize(FONT_SIZE))
        .build()
        .unwrap()
        .size()
        .width + 5.0 // because
}

fn line_for_idx(idx: usize, layout: &impl TextLayout) -> Line {
    let pos = layout.hit_test_text_position(idx);
    let line_metrics = layout.line_metric(pos.line).unwrap();
    let p1 = (pos.point.x, line_metrics.y_offset);
    let p2 = (pos.point.x, (line_metrics.y_offset + line_metrics.height));
    Line::new(p1, p2)
}

fn text_label_for_idx<T: Text>(idx: usize, text: &mut T) -> T::TextLayout {
    text.new_text_layout(idx.to_string())
        .default_attribute(TextAttribute::FontSize(10.0))
        .build()
        .unwrap()
}
