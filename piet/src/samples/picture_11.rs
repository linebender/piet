//! Visualize results of hit-testing

use crate::kurbo::{Circle, Line, Point, Size, Vec2};
use crate::{Color, Error, RenderContext, Text, TextLayout, TextLayoutBuilder};

pub const SIZE: Size = Size::new(480., 800.);
pub const DOT_RADIUS: f64 = 2.0;
pub const TEST_ADVANCE: f64 = 23.4;

static TEXT: &str = r#"Philosophers often behave like little children who scribble some marks on a piece of paper at random and then ask the grown-up "What's that?" — It happened like this: the grown-up had drawn pictures for the child several times and said "this is a man," "this is a house," etc. And then the child makes some marks too and asks: what's this then?"#;

const LIGHT_GREY: Color = Color::grey8(0xc0);
const RED: Color = Color::rgb8(255, 0, 0);
const BLUE: Color = Color::rgb8(0, 0, 255);

pub fn draw<R: RenderContext>(rc: &mut R) -> Result<(), Error> {
    rc.clear(LIGHT_GREY);
    let text = rc.text();
    let layout = text.new_text_layout(TEXT).max_width(200.0).build()?;

    let y_pos = ((SIZE.height - layout.size().height * 2.0) / 4.0).max(0.0);

    let text_pos = Vec2::new(16.0, y_pos);
    let layout_rect = layout.size().to_rect() + text_pos;
    rc.fill(layout_rect, &Color::WHITE);

    let mut y = y_pos - 20.;
    while y < (layout_rect.max_y() + TEST_ADVANCE) {
        let mut x = 2.0;
        while x < SIZE.width / 2.0 {
            let point = Point::new(x, y);
            let test_point = layout.hit_test_point(point - text_pos);
            let test_pos = layout.hit_test_text_position(test_point.idx);
            let hit_point = test_pos.point + text_pos;

            let color = if test_point.is_inside { &RED } else { &BLUE };

            let line = Line::new(point, hit_point);
            let dot = Circle::new(hit_point, DOT_RADIUS);
            rc.stroke(line, color, 0.5);
            rc.fill(dot, color);
            x += TEST_ADVANCE;
        }
        y += TEST_ADVANCE;
    }
    rc.draw_text(&layout, text_pos.to_point());

    Ok(())
}
