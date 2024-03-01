// Copyright 2019 the Piet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Basic example of rendering on Direct2D.

use std::path::Path;

use piet::{samples, RenderContext};
use piet_common::Device;

const FILE_PREFIX: &str = "d2d-test";

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

    // We need to postpone returning a potential error to ensure cleanup
    let draw_error = sample.draw(&mut piet_context).err();

    piet_context.finish()?;
    std::mem::drop(piet_context);

    // Return either the draw error, or the result of the attempt to save the file
    draw_error.map_or_else(
        || target.save_to_file(save_path).map_err(Into::into),
        |e| Err(e.into()),
    )
}
