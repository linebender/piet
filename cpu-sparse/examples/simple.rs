use std::io::BufWriter;

use cpu_sparse::{CsRenderCtx, Pixmap};
use piet_next::peniko::color::palette;
use piet_next::peniko::kurbo::BezPath;
use piet_next::RenderCtx;

const WIDTH: usize = 1024;
const HEIGHT: usize = 256;

pub fn main() {
    let mut ctx = CsRenderCtx::new(WIDTH, HEIGHT);
    let mut path = BezPath::new();
    path.move_to((10.0, 10.0));
    path.line_to((900.0, 20.0));
    path.line_to((150.0, 200.0));
    path.close_path();
    ctx.fill(&path.into(), palette::css::REBECCA_PURPLE.into());
    if let Some(filename) = std::env::args().nth(1) {
        let mut pixmap = Pixmap::new(WIDTH, HEIGHT);
        ctx.render_to_pixmap(&mut pixmap);
        pixmap.unpremultiply();
        let file = std::fs::File::create(filename).unwrap();
        let w = BufWriter::new(file);
        let mut encoder = png::Encoder::new(w, WIDTH as u32, HEIGHT as u32);
        encoder.set_color(png::ColorType::Rgba);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(pixmap.data()).unwrap();
    } else {
        ctx.debug_dump();
    }
}
