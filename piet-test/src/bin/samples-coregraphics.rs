//! Run the piet-test examples with the coregraphics backend.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use core_graphics::color_space::CGColorSpace;
use core_graphics::context::CGContext;

use piet::kurbo::Size;
use piet::RenderContext;
use piet_coregraphics::CoreGraphicsContext;
use piet_test::samples;

const SCALE: f64 = 2.0;
const FILE_PREFIX: &str = "coregraphics-test-";

fn main() {
    samples::samples_main("samples-coregraphics", run_sample, FILE_PREFIX, None);
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

    let mut raw_pixels = cg_ctx.data().to_vec();
    piet_coregraphics::unpremultiply_rgba(&mut raw_pixels);

    image::save_buffer(
        &path,
        &raw_pixels,
        size.width as u32,
        size.height as u32,
        image::ColorType::Rgba8,
    )?;
    Ok(())
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
