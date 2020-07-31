//! range attributes should override default attributes

use crate::kurbo::{Size, Vec2};
use crate::{
    Color, Error, FontBuilder, FontWeight, RenderContext, Text, TextAttribute, TextLayout,
    TextLayoutBuilder,
};

pub const SIZE: Size = Size::new(480., 560.);

static TEXT: &str = r#"Philosophers often behave like little children who scribble some marks on a piece of paper at random and then ask the grown-up "What's that?" — It happened like this: the grown-up had drawn pictures for the child several times and said "this is a man," "this is a house," etc. And then the child makes some marks too and asks: what's this then?"#;

const RED: Color = Color::rgb8(255, 0, 0);
const BLUE: Color = Color::rgb8(0, 0, 255);

pub fn draw<R: RenderContext>(rc: &mut R) -> Result<(), Error> {
    rc.clear(Color::WHITE);
    let text = rc.text();
    let font = text.system_font(12.0);
    let font2 = text.new_font_by_name("Courier New", 12.0).build().unwrap();
    let layout = text
        .new_text_layout(TEXT)
        .max_width(200.0)
        .default_attribute(font2)
        .default_attribute(TextAttribute::Underline(true))
        .default_attribute(TextAttribute::Italic(true))
        .default_attribute(TextAttribute::ForegroundColor(RED))
        .default_attribute(FontWeight::BOLD)
        .range_attribute(..200, TextAttribute::ForegroundColor(BLUE))
        .range_attribute(10..100, FontWeight::NORMAL)
        .range_attribute(40..300, TextAttribute::Underline(false))
        .range_attribute(60..160, TextAttribute::Italic(false))
        .range_attribute(140..220, FontWeight::NORMAL)
        .range_attribute(240.., font)
        .build()?;

    let y_pos = ((SIZE.height - layout.size().height * 2.0) / 4.0).max(0.0);
    let text_pos = Vec2::new(16.0, y_pos);
    rc.draw_text(&layout, text_pos.to_point());

    Ok(())
}
