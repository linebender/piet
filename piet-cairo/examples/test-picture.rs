//! Basic example of rendering on Cairo.

use std::fs::File;
use std::path::Path;

use cairo::{Context, Format, ImageSurface};

use piet::{samples, RenderContext};
use piet_cairo::CairoRenderContext;

const HIDPI: f64 = 2.0;
const FILE_PREFIX: &str = "cairo-test-";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    samples::samples_main(run_sample)
}

fn run_sample(idx: usize, base_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let sample = samples::get(idx);
    let size = sample.size();

    let file_name = format!("{}{}.png", FILE_PREFIX, idx);
    let path = base_dir.join(file_name);

    let surface = ImageSurface::create(Format::ARgb32, size.width as i32, size.height as i32)
        .expect("Can't create surface");
    let cr = Context::new(&surface);
    cr.scale(HIDPI, HIDPI);
    let mut piet_context = CairoRenderContext::new(&cr);
    sample.draw(&mut piet_context)?;
    piet_context.finish()?;
    surface.flush();

    let mut file = File::create(path)?;

    surface.write_to_png(&mut file).map_err(Into::into)
}
