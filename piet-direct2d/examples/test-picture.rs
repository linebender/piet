//! Basic example of rendering on Direct2D.

use std::path::Path;

use piet::{samples, RenderContext};
use piet_common::Device;

// TODO: Improve support for fractional scaling where sample size ends up fractional.
const SCALE: f64 = 2.0;
const FILE_PREFIX: &str = "d2d-test-";

fn main() {
    samples::samples_main(run_sample, FILE_PREFIX, None);
}

fn run_sample(idx: usize, base_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let sample = samples::get(idx)?;
    let size = sample.size() * SCALE;

    let file_name = format!("{}{}.png", FILE_PREFIX, idx);
    let path = base_dir.join(file_name);

    let mut device = Device::new()?;
    let mut target = device.bitmap_target(size.width as usize, size.height as usize, SCALE)?;
    let mut piet_context = target.render_context();

    // We need to postpone returning a potential error to ensure cleanup
    let draw_error = sample.draw(&mut piet_context).err();

    piet_context.finish()?;
    std::mem::drop(piet_context);

    // Return either the draw error, or the result of the attempt to save the file
    draw_error.map_or_else(
        || target.save_to_file(path).map_err(Into::into),
        |e| Err(e.into()),
    )
}
