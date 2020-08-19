//! Verifying line metrics and text layout geometry behave as intended.

use crate::kurbo::{Line, Size, Vec2};
use crate::{
    Color, Error, FontFamily, RenderContext, Text, TextAlignment, TextAttribute, TextLayout,
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
    let courier = text
        .font_family("Courier New")
        .unwrap_or(FontFamily::MONOSPACE);
    let layout = text
        .new_text_layout(SAMPLE_EN)
        .max_width(200.0)
        .alignment(TextAlignment::Start)
        .font(FontFamily::SYSTEM_UI, 24.0)
        .range_attribute(23..35, courier.clone())
        .range_attribute(23..35, TextAttribute::FontSize(18.0))
        .range_attribute(47..52, courier)
        .range_attribute(47..52, TextAttribute::FontSize(36.0))
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
