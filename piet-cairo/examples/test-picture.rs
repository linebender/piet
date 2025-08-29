// Copyright 2019 the Piet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Basic example of rendering on Cairo.

use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use piet::{RenderContext, samples};
use piet_common::Device;

const FILE_PREFIX: &str = "cairo-test";

fn main() {
    let sys_info = additional_system_info();
    samples::samples_main(run_sample, FILE_PREFIX, Some(&sys_info));
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

fn additional_system_info() -> String {
    let mut r = String::new();
    append_lib_version("libpango1.0", &mut r);
    append_lib_version("libcairo2", &mut r);
    r
}

fn append_lib_version(package_name: &str, buf: &mut String) {
    let version = get_version_from_apt(package_name);
    write!(buf, "{package_name:16}").expect("Failed to write package name to string");
    buf.push_str(version.as_deref().unwrap_or("not found"));
    buf.push('\n')
}

fn get_version_from_apt(package: &str) -> Option<String> {
    let output = match Command::new("aptitude")
        .arg("show")
        .arg(package)
        .output()
        .or_else(|_| Command::new("apt-cache").arg("show").arg(package).output())
    {
        Ok(output) => output,
        Err(e) => {
            eprintln!("failed to get package version: '{e}'");
            return None;
        }
    };

    let output = if output.status.success() {
        String::from_utf8(output.stdout).expect("malformed utf8")
    } else {
        eprintln!("apt-cache failed {:?}", &output);
        return None;
    };

    output
        .lines()
        .find(|s| s.trim().starts_with("Version"))
        .and_then(|line| line.split(':').next_back().map(|s| s.trim().to_owned()))
}
