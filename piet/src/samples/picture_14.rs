//! Setting font weight when a variable font has a 'wght' axis

use crate::kurbo::{Size, Vec2};
use crate::{
    Color, Error, FontFamily, FontWeight, RenderContext, Text, TextLayout, TextLayoutBuilder,
};

pub const SIZE: Size = Size::new(240., 280.);

static TEXT: &str = r#"100200300400500
600700800900950"#;

pub fn draw<R: RenderContext>(rc: &mut R) -> Result<(), Error> {
    rc.clear(None, Color::WHITE);
    let text = rc.text();
    let font = text
        .load_font(include_bytes!(
            "../../snapshots/resources/Inconsolata-variable.ttf"
        ))
        .unwrap_or(FontFamily::SYSTEM_UI);

    let layout = text
        .new_text_layout(TEXT)
        .max_width(200.0)
        .font(font, 24.0)
        .range_attribute(..3, FontWeight::THIN)
        .range_attribute(3..6, FontWeight::EXTRA_LIGHT)
        .range_attribute(6..9, FontWeight::LIGHT)
        .range_attribute(9..12, FontWeight::REGULAR)
        .range_attribute(12..15, FontWeight::MEDIUM)
        .range_attribute(16..19, FontWeight::SEMI_BOLD)
        .range_attribute(19..22, FontWeight::BOLD)
        .range_attribute(22..25, FontWeight::EXTRA_BOLD)
        .range_attribute(25..28, FontWeight::BLACK)
        .range_attribute(28..31, FontWeight::EXTRA_BLACK)
        .build()?;

    let y_pos = ((SIZE.height - layout.size().height * 2.0) / 4.0).max(0.0);
    let text_pos = Vec2::new(16.0, y_pos);
    rc.draw_text(&layout, text_pos.to_point());

    Ok(())
}
