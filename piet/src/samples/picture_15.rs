//! Setting font weight when a variable font has a 'wght' axis

use crate::kurbo::{Size, Vec2};
use crate::{
    Color, Error, FontFamily, RenderContext, Text, TextAttribute, TextLayout, TextLayoutBuilder,
};

pub const SIZE: Size = Size::new(480., 560.);

static TEXT: &str = r#"100200300400500
600700800900950"#;

pub fn draw<R: RenderContext>(rc: &mut R) -> Result<(), Error> {
    rc.clear(Color::WHITE);
    let text = rc.text();
    let font = text
        .load_font(include_bytes!(
            "../../../snapshots/resources/Inconsolata-variable.ttf"
        ))
        .unwrap_or(FontFamily::SYSTEM_UI);

    let layout = text
        .new_text_layout(TEXT)
        .max_width(200.0)
        .font(font, 24.0)
        .range_attribute(..3, TextAttribute::FontSize(-12.0))
        .range_attribute(3..6, TextAttribute::FontSize(0.0))
        .range_attribute(6..9, TextAttribute::FontSize(0.1))
        .range_attribute(9..12, TextAttribute::FontSize(1.0))
        .range_attribute(12..15, TextAttribute::FontSize(4.0))
        .build()?;

    let y_pos = ((SIZE.height - layout.size().height * 2.0) / 4.0).max(0.0);
    let text_pos = Vec2::new(16.0, y_pos);
    rc.draw_text(&layout, text_pos.to_point());

    Ok(())
}
