//! Basic example of rendering on Cairo.

use std::fs::File;

use cairo::{Context, Format, ImageSurface};

use piet::draw_test_picture;
use piet::RenderContext;
use piet_cairo::CairoRenderContext;

const HIDPI: f64 = 2.0;

fn main() {
    let test_picture_number = std::env::args()
        .nth(1)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
    let size = piet::size_for_test_picture(test_picture_number).unwrap();

    let surface = ImageSurface::create(Format::ARgb32, size.width as i32, size.height as i32)
        .expect("Can't create surface");
    let mut cr = Context::new(&surface);
    cr.scale(HIDPI, HIDPI);
    let mut piet_context = CairoRenderContext::new(&mut cr);
    draw_test_picture(&mut piet_context, test_picture_number).unwrap();
    piet_context.finish().unwrap();
    surface.flush();
    let mut file = File::create(format!("cairo-test-{}.png", test_picture_number)).unwrap();
    surface
        .write_to_png(&mut file)
        .expect("Error writing image file");
}
