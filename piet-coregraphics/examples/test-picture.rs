//! Run the piet-test examples with the coregraphics backend.

use std::path::Path;

use piet::{samples, RenderContext};
use piet_common::Device;

const FILE_PREFIX: &str = "coregraphics-test";

fn main() {
    samples::samples_main(run_sample, FILE_PREFIX, None);
}

fn run_sample(
    number: usize,
    scale: f64,
    save_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let sample = samples::get(number)?;
    let size = sample.size() * scale;

    let mut device = Device::new()?;
    let mut target = device.bitmap_target(size.width as usize, size.height as usize, scale)?;
    let mut piet_context = target.render_context();

    sample.draw(&mut piet_context)?;

    piet_context.finish()?;
    std::mem::drop(piet_context);

    target.save_to_file(save_path).map_err(Into::into)
}
