//! Show the relationship between the layout rect and the inking/image rect.

use crate::kurbo::{Size, Vec2};
use crate::{Color, Error, RenderContext, Text, TextLayout, TextLayoutBuilder};

pub const SIZE: Size = Size::new(400., 400.);

static SAMPLE_EN: &str = r#"ḧ́ͥm̾ͭpͭ̒ͦ̎ḧ̐̈̾̆͊
 ch̯͈̙̯̼̠a͚͉o̺̮̳̮̩s̪͇.̥̩̹"#;

const LIGHT_GREY: Color = Color::grey8(0xc0);

pub fn draw<R: RenderContext>(rc: &mut R) -> Result<(), Error> {
    rc.clear(LIGHT_GREY);
    let text = rc.text();
    let font = text.system_font(24.0);

    let layout = text.new_text_layout(&font, SAMPLE_EN, None).build()?;

    let text_pos = Vec2::new(50.0, 50.0);
    let layout_rect = layout.size().to_rect() + text_pos;
    let image_rect = layout.image_bounds() + text_pos;

    rc.fill(layout_rect, &Color::WHITE);
    rc.stroke(image_rect, &Color::BLACK, 0.5);

    rc.draw_text(&layout, text_pos.to_point(), &Color::BLACK);

    Ok(())
}
