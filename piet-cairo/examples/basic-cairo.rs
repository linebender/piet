//! Basic example of rendering on Cairo.

use std::fs::File;

use cairo::{Context, Format, ImageSurface};

use piet::RenderContext;
use piet_cairo::CairoRenderContext;

use piet_test::draw_test_picture;

const TEXTURE_WIDTH: i32 = 400;
const TEXTURE_HEIGHT: i32 = 200;

const HIDPI: f64 = 2.0;

fn main() {
    let test_picture_number = std::env::args()
        .skip(1)
        .next()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);

    let surface = ImageSurface::create(Format::ARgb32, TEXTURE_WIDTH, TEXTURE_HEIGHT)
        .expect("Can't create surface");
    let mut cr = Context::new(&surface);
    cr.scale(HIDPI, HIDPI);
    let mut piet_context = CairoRenderContext::new(&mut cr);
    draw_test_picture(&mut piet_context, test_picture_number).unwrap();
    piet_context.finish().unwrap();
    surface.flush();
    let mut file = File::create("temp-cairo.png").expect("Couldn't create 'file.png'");
    surface
        .write_to_png(&mut file)
        .expect("Error writing image file");
}
