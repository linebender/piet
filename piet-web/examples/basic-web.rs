//! Basic example of rendering on Cairo.

use kurbo::{BezPath, Line};

use stdweb::traits::*;
use stdweb::unstable::TryInto;
use stdweb::web::{document, CanvasRenderingContext2d};

use stdweb::web::html_element::CanvasElement;

use piet::{FillRule, FontBuilder, RenderContext, TextLayoutBuilder};
use piet_web::WebRenderContext;

fn draw_pretty_picture<R: RenderContext>(rc: &mut R) {
    rc.clear(0xFF_FF_FF);
    let brush = rc.solid_brush(0x00_00_80_FF);
    rc.stroke(&Line::new((10.0, 10.0), (100.0, 50.0)), &brush, 1.0, None);

    let mut path = BezPath::new();
    path.moveto((50.0, 10.0));
    path.quadto((60.0, 50.0), (100.0, 90.0));
    let brush = rc.solid_brush(0x00_80_00_FF);
    rc.stroke(&path, &brush, 1.0, None);

    let mut path = BezPath::new();
    path.moveto((10.0, 20.0));
    path.curveto((10.0, 80.0), (100.0, 80.0), (100.0, 60.0));
    let brush = rc.solid_brush(0x00_00_80_C0);
    rc.fill(&path, &brush, FillRule::NonZero);

    let font = rc.new_font_by_name("Segoe UI", 12.0).build();
    let layout = rc.new_text_layout(&font, "Hello piet-web!").build();
    let brush = rc.solid_brush(0x80_00_00_C0);
    rc.draw_text(&layout, (80.0, 10.0), &brush);
}

fn main() {
    stdweb::initialize();

    let canvas: CanvasElement = document()
        .query_selector("#canvas")
        .unwrap()
        .unwrap()
        .try_into()
        .unwrap();
    let mut context: CanvasRenderingContext2d = canvas.get_context().unwrap();
    let hidpi = 2; // TODO: fetch this from environment.

    canvas.set_width(hidpi * canvas.offset_width() as u32);
    canvas.set_height(hidpi * canvas.offset_height() as u32);
    context.scale(hidpi as f64, hidpi as f64);

    let mut piet_context = WebRenderContext::new(&mut context);
    draw_pretty_picture(&mut piet_context);

    stdweb::event_loop();
}
