//! Verifying line metrics and text layout geometry behave as intended.

use crate::kurbo::{Line, Size, Vec2};
use crate::{
    Color, Error, FontBuilder, RenderContext, Text, TextAlignment, TextAttribute, TextLayout,
    TextLayoutBuilder,
};

pub const SIZE: Size = Size::new(464., 800.);

static SAMPLE_EN: &str = r#"1nnyyÊbbbb soft break
2nnyyÊbbbb
3nnyyÊbbbb
4nnyyÊbbbb
5nnyyÊbbbb
6nnyyÊbbbb
7nnyyÊbbbb
"#;

const LIGHT_GREY: Color = Color::grey8(0xc0);
const RED: Color = Color::rgb8(255, 0, 0);
const BLUE: Color = Color::rgb8(0, 0, 255);
const GREEN: Color = Color::rgb8(105, 255, 0);

pub fn draw<R: RenderContext>(rc: &mut R) -> Result<(), Error> {
    rc.clear(LIGHT_GREY);
    let text = rc.text();
    let font = text.system_font(24.0);
    let mono = text
        .new_font_by_name("Courier New", 18.0)
        .build()
        .expect("missing Courier New");

    let layout = text
        .new_text_layout(&font, SAMPLE_EN, 200.0)
        .alignment(TextAlignment::Start)
        .add_attribute(23..35, mono.clone())
        .add_attribute(23..35, TextAttribute::Size(18.0))
        .add_attribute(47..52, mono)
        .add_attribute(47..52, TextAttribute::Size(36.0))
        .build()?;

    let text_pos = Vec2::new(0.0, 50.0);
    let layout_rect = layout.size().to_rect() + text_pos;
    let image_rect = layout.image_bounds() + text_pos;

    rc.fill(layout_rect, &Color::WHITE);
    rc.stroke(image_rect, &Color::BLACK, 0.5);

    for idx in 0..layout.line_count() {
        let metrics = layout.line_metric(idx).unwrap();
        let line_width = 200.0;

        let top = text_pos.y + metrics.y_offset;
        let line_top = Line::new((0.0, top), (line_width, top));

        let baseline = metrics.y_offset + metrics.baseline + text_pos.y;
        let baseline_line = Line::new((0., baseline), (line_width, baseline));

        let bottom = metrics.y_offset + metrics.height + text_pos.y;
        let bottom_line = Line::new((0., bottom), (line_width, bottom));

        rc.stroke(line_top, &RED, 1.0);
        rc.stroke(baseline_line, &GREEN, 1.0);
        rc.stroke(bottom_line, &BLUE, 1.0);
    }

    rc.draw_text(&layout, text_pos.to_point());

    Ok(())
}
