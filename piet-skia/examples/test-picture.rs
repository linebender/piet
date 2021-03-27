use std::fs::File;
use std::io::Write;
use std::path::Path;

use piet::kurbo::Circle;
use piet::{samples, RenderContext};
use piet_skia::SkiaRenderContext;
use skia_safe::{EncodedImageFormat, Surface};

const HIDPI: f64 = 2.0;
const FILE_PREFIX: &str = "skia-test-";

fn main() {
    //let height = 100i32;
    //let width = 100i32;
    //let mut surface = Surface::new_raster_n32_premul((width, height)).expect("No surface!");
    //let canvas = surface.canvas();
    //let mut skia_ctx = SkiaRenderContext::new(canvas);
    //skia_ctx.stroke(Circle::new((10., 10.), 10.), &piet::Color::rgba8(100, 100, 100, 255), 10.);
    //let image = surface.image_snapshot();
    //let data = image.encode_to_data(EncodedImageFormat::PNG).expect("Failed to encode data");
    //let mut file = File::create("test.png").unwrap();
    //let bytes = data.as_bytes();
    //file.write_all(bytes).unwrap();
    samples::samples_main(run_sample, FILE_PREFIX)
}

fn run_sample(idx: usize, base_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let sample = samples::get(idx)?;
    let size = sample.size();
    let file_name = format!("{}{}.png", FILE_PREFIX, idx);
    let path = base_dir.join(file_name);

    let (width, height) = (size.width as i32, size.height as i32);
    let mut surface = Surface::new_raster_n32_premul((width, height)).expect("No surface!");
    let canvas = surface.canvas();
    canvas.scale((HIDPI as f32, HIDPI as f32));
    let mut piet_ctx = SkiaRenderContext::new(canvas);

    sample.draw(&mut piet_ctx)?;
    piet_ctx.finish()?;
    surface.flush();

    let image = surface.image_snapshot();
    let data = image
        .encode_to_data(EncodedImageFormat::PNG)
        .expect("Failed to encode data");
    let mut file = File::create(path)?;
    let bytes = data.as_bytes();
    file.write_all(bytes).map_err(Into::into)

    //surface.write_to_png(&mut file).map_err(Into::into)
}
