//! range attributes should override default attributes

use crate::kurbo::{Size, Vec2};
use crate::{
    Color, Error, FontFamily, RenderContext, Text, TextAttribute, TextLayout, TextLayoutBuilder,
};

pub const SIZE: Size = Size::new(480., 560.);

static TEXT: &str = r#"The idea of "structurelessness," however, has moved from a healthy counter to those tendencies to becoming a goddess in its own right. The idea is as little examined as the term is much used, but it has become an intrinsic and unquestioned part of women's liberation ideology. For the early development of the movement this did not much matter."#;

const SELECTION_COLOR: Color = Color::rgb8(165, 205, 255);
const HILIGHT_COLOR: Color = Color::rgba8(255, 242, 54, 96);

pub fn draw<R: RenderContext>(rc: &mut R) -> Result<(), Error> {
    rc.clear(None, Color::WHITE);
    let text = rc.text();
    let font2 = text.font_family("Courier New").unwrap();
    let layout = text
        .new_text_layout(TEXT)
        .max_width(200.0)
        .font(FontFamily::SYSTEM_UI, 12.0)
        .range_attribute(280.., font2)
        .range_attribute(280.., TextAttribute::FontSize(18.0))
        .build()?;

    let y_pos = ((SIZE.height - layout.size().height * 2.0) / 4.0).max(0.0);
    let text_pos = Vec2::new(16.0, y_pos);

    let sel_one = layout.rects_for_range(10..72);
    let sel_two = layout.rects_for_range(240..);
    dbg!(&sel_one, &sel_two);

    for rect in sel_one {
        rc.fill(rect + text_pos, &SELECTION_COLOR);
    }

    rc.draw_text(&layout, text_pos.to_point());

    for rect in sel_two {
        rc.fill(rect + text_pos, &HILIGHT_COLOR);
    }

    Ok(())
}
