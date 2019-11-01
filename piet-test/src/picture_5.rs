//! Basic example of just text

use piet::kurbo::Line;
use piet::{Color, Error, FontBuilder, RenderContext, Text, TextLayout, TextLayoutBuilder};

pub fn draw(rc: &mut impl RenderContext) -> Result<(), Error> {
    // Black background
    rc.clear(Color::BLACK);

    // do something texty
    let font = rc.text().new_font_by_name("Segoe UI", 12.0).build()?;

    let layout = rc.text().new_text_layout(&font, "piet text!").build()?;

    let width = layout.width();

    let brush = rc.solid_brush(Color::rgba8(0x00, 0x80, 0x80, 0xF0));

    rc.draw_text(&layout, (100.0, 50.0), &brush);

    // underline text
    rc.stroke(Line::new((100.0, 52.0), (100.0 + width, 52.0)), &brush, 1.0);

    // get hit test text position and draw a cursor
    let hit_test_text_position = layout.hit_test_text_position(3);

    if let Some(http) = hit_test_text_position {
        let cursor_x = http.point.x;

        let color = Color::rgba8(0x80, 0x80, 0x80, 0xF0);

        rc.stroke(
            Line::new((100.0 + cursor_x, 50.0), (100.0 + cursor_x, 50.0 - 10.0)),
            &color,
            1.0,
        );
    }

    Ok(())
}
