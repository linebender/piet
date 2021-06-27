//! Basic example of rendering on Cairo.

use std::fs::File;
use std::path::Path;
use std::process::Command;

use cairo::{Context, Format, ImageSurface};

use piet::{samples, RenderContext};
use piet_cairo::CairoRenderContext;

const HIDPI: f64 = 2.0;
const FILE_PREFIX: &str = "cairo-test-";

fn main() {
    let sys_info = additional_system_info();
    samples::samples_main(run_sample, FILE_PREFIX, Some(&sys_info));
}

fn run_sample(idx: usize, base_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let sample = samples::get(idx)?;
    let size = sample.size();

    let file_name = format!("{}{}.png", FILE_PREFIX, idx);
    let path = base_dir.join(file_name);

    let surface = ImageSurface::create(Format::ARgb32, size.width as i32, size.height as i32)
        .expect("Can't create surface");
    let cr = Context::new(&surface).unwrap();
    cr.scale(HIDPI, HIDPI);
    let mut piet_context = CairoRenderContext::new(&cr);
    sample.draw(&mut piet_context)?;
    piet_context.finish()?;
    surface.flush();

    let mut file = File::create(path)?;

    surface.write_to_png(&mut file).map_err(Into::into)
}

fn additional_system_info() -> String {
    let mut r = String::new();
    append_lib_version("libpango1.0", &mut r);
    append_lib_version("libcairo2", &mut r);
    r
}

fn append_lib_version(package_name: &str, buf: &mut String) {
    let version = get_version_from_apt(package_name);
    buf.push_str(&format!("{:16}", package_name));
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
            eprintln!("failed to get package version: '{}'", e);
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
        .and_then(|line| line.split(':').last().map(|s| s.trim().to_owned()))
}
