//! Basic example of rendering on Cairo.

use kurbo::BezPath;

use stdweb::traits::*;
use stdweb::unstable::TryInto;
use stdweb::web::{document, CanvasRenderingContext2d};

use stdweb::web::html_element::CanvasElement;

use piet::{FillRule, RenderContext};
use piet_web::WebRenderContext;

fn draw_pretty_picture<R: RenderContext>(rc: &mut R) {
    rc.clear(0xFF_FF_FF);
    let brush = rc.solid_brush(0x00_00_80_FF);
    rc.line((10.0, 10.0), (100.0, 50.0), &brush, 1.0, None);

    let mut path = BezPath::new();
    path.moveto((50.0, 10.0));
    path.quadto((60.0, 50.0), (100.0, 90.0));
    let brush = rc.solid_brush(0x00_80_00_FF);
    rc.stroke_path(path.elements().iter().cloned(), &brush, 1.0, None);

    let mut path = BezPath::new();
    path.moveto((10.0, 20.0));
    path.curveto((10.0, 80.0), (100.0, 80.0), (100.0, 60.0));
    let brush = rc.solid_brush(0x00_00_80_C0);
    // We'll make this `&path` by fixing kurbo.
    rc.fill_path(path.elements(), &brush, FillRule::NonZero);
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

    canvas.set_width(canvas.offset_width() as u32);
    canvas.set_height(canvas.offset_height() as u32);

    let mut piet_context = WebRenderContext::new(&mut context);
    draw_pretty_picture(&mut piet_context);

    stdweb::event_loop();
}
