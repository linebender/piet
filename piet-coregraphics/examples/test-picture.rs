//! Run the piet-test examples with the coregraphics backend.

use std::path::Path;

use piet::{samples, RenderContext};
use piet_common::Device;

// TODO: Improve support for fractional scaling where sample size ends up fractional.
const SCALE: f64 = 2.0;
const FILE_PREFIX: &str = "coregraphics-test-";

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

    sample.draw(&mut piet_context)?;

    piet_context.finish()?;
    std::mem::drop(piet_context);

    target.save_to_file(path).map_err(Into::into)
}
