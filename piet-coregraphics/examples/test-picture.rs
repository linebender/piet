//! Run the piet-test examples with the coregraphics backend.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use core_graphics::color_space::CGColorSpace;
use core_graphics::context::CGContext;

use piet::kurbo::Size;
use piet::{samples, RenderContext};
use piet_coregraphics::CoreGraphicsContext;

const SCALE: f64 = 2.0;
const FILE_PREFIX: &str = "coregraphics-test-";

fn main() {
    samples::samples_main(run_sample, FILE_PREFIX, None);
}

fn run_sample(idx: usize, base_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let sample = samples::get(idx)?;
    let size = sample.size();

    let file_name = format!("{}{}.png", FILE_PREFIX, idx);
    let path = base_dir.join(file_name);

    let mut cg_ctx = make_cg_ctx(size);
    let mut piet_context =
        CoreGraphicsContext::new_y_up(&mut cg_ctx, size.height * SCALE.recip(), None);

    sample.draw(&mut piet_context)?;

    piet_context.finish()?;
    std::mem::drop(piet_context);
    let mut data = cg_ctx.data().to_vec();
    let file = File::create(path)?;
    let w = BufWriter::new(file);

    let mut encoder = png::Encoder::new(w, size.width as u32, size.height as u32);
    encoder.set_color(png::ColorType::RGBA);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;

    piet_coregraphics::unpremultiply_rgba(&mut data);
    writer.write_image_data(&data).map_err(Into::into)
}

fn make_cg_ctx(size: Size) -> CGContext {
    let cg_ctx = CGContext::create_bitmap_context(
        None,
        size.width as usize,
        size.height as usize,
        8,
        0,
        &CGColorSpace::create_device_rgb(),
        core_graphics::base::kCGImageAlphaPremultipliedLast,
    );
    cg_ctx.scale(SCALE, SCALE);
    cg_ctx
}
