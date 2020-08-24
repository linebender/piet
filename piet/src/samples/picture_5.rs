//! range attributes should override default attributes

use crate::kurbo::{Size, Vec2};
use crate::{
    Color, Error, FontFamily, FontWeight, RenderContext, Text, TextAttribute, TextLayout,
    TextLayoutBuilder,
};

pub const SIZE: Size = Size::new(480., 560.);

static TEXT: &str = "Philosophers often behave like little children who scribble \
                    some marks on a piece of paper at random and then ask the \
                    grown-up 'What's that?'";
const RED: Color = Color::rgb8(255, 0, 0);
const BLUE: Color = Color::rgb8(0, 0, 255);

pub fn draw<R: RenderContext>(rc: &mut R) -> Result<(), Error> {
    rc.clear(Color::WHITE);
    let text = rc.text();
    let sans = text
        .font_family("Helvetica")
        .or_else(|| text.font_family("Arial"))
        .unwrap_or(FontFamily::SANS_SERIF);
    let serif = text.font_family("Georgia").unwrap_or(FontFamily::SERIF);

    let layout = text
        .new_text_layout(TEXT)
        .max_width(200.0)
        .default_attribute(serif)
        .default_attribute(TextAttribute::FontSize(12.0))
        .range_attribute(..13, TextAttribute::ForegroundColor(BLUE))
        .range_attribute(..13, TextAttribute::FontSize(24.0))
        .range_attribute(14..19, TextAttribute::Italic(true))
        .range_attribute(30.., FontWeight::NORMAL)
        .range_attribute(31..47, TextAttribute::FontSize(8.0))
        .range_attribute(60..70, sans.clone())
        .range_attribute(60..70, FontWeight::BLACK)
        .range_attribute(90..100, sans)
        .range_attribute(90..100, FontWeight::LIGHT)
        .range_attribute(90..100, TextAttribute::ForegroundColor(RED))
        .range_attribute(118..126, TextAttribute::Underline(true))
        .range_attribute(128..140, TextAttribute::Italic(true))
        .range_attribute(135..140, FontWeight::BOLD)
        .build()?;

    let y_pos = ((SIZE.height - layout.size().height * 2.0) / 4.0).max(0.0);
    let text_pos = Vec2::new(16.0, y_pos);
    rc.draw_text(&layout, text_pos.to_point());

    Ok(())
}
