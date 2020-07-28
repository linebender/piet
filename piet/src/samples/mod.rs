//! Drawing examples for testing backends

use std::path::{Path, PathBuf};

use crate::kurbo::Size;
use crate::{Error, RenderContext};

mod picture_0;
mod picture_1;
mod picture_2;
mod picture_3;
mod picture_4;
mod picture_5;
mod picture_6;
mod picture_7;
mod picture_8;
mod picture_9;

mod picture_10;
mod picture_11;

type BoxErr = Box<dyn std::error::Error>;

/// The total number of samples in this module.
pub const SAMPLE_COUNT: usize = 12;

/// Return a specific sample for drawing.
pub fn get<R: RenderContext>(number: usize) -> SamplePicture<R> {
    match number {
        0 => SamplePicture::new(picture_0::SIZE, picture_0::draw),
        1 => SamplePicture::new(picture_1::SIZE, picture_1::draw),
        2 => SamplePicture::new(picture_2::SIZE, picture_2::draw),
        3 => SamplePicture::new(picture_3::SIZE, picture_3::draw),
        4 => SamplePicture::new(picture_4::SIZE, picture_4::draw),
        5 => SamplePicture::new(picture_5::SIZE, picture_5::draw),
        6 => SamplePicture::new(picture_6::SIZE, picture_6::draw),
        7 => SamplePicture::new(picture_7::SIZE, picture_7::draw),
        8 => SamplePicture::new(picture_8::SIZE, picture_8::draw),
        9 => SamplePicture::new(picture_9::SIZE, picture_9::draw),
        10 => SamplePicture::new(picture_10::SIZE, picture_10::draw),
        11 => SamplePicture::new(picture_11::SIZE, picture_11::draw),
        _ => panic!("No sample #{} exists", number),
    }
}

/// A pointer to a text drawing and associated info.
pub struct SamplePicture<T> {
    draw_f: fn(&mut T) -> Result<(), Error>,
    size: Size,
}

/// Arguments used by backend cli utilities.
struct Args {
    all: bool,
    out_dir: PathBuf,
    number: Option<usize>,
}

/// A shared `main` fn for diferent backends.
///
/// The important thing here is the fn argument; this should be a method that
/// takes a number and a path, executes the corresponding sample, and saves a
/// PNG to the path.
pub fn samples_main(f: fn(usize, &Path) -> Result<(), BoxErr>) -> Result<(), BoxErr> {
    let args = Args::from_env()?;

    if !args.out_dir.exists() {
        std::fs::create_dir_all(&args.out_dir)?;
    }

    if args.all {
        run_all(|number| f(number, &args.out_dir))?;
    } else if let Some(number) = args.number {
        f(number, &args.out_dir)?;
    }

    Ok(())
}

impl<T> SamplePicture<T> {
    fn new(size: Size, draw_f: fn(&mut T) -> Result<(), Error>) -> Self {
        SamplePicture { size, draw_f }
    }

    /// The size of the context expected by this sample, in pixels.
    pub fn size(&self) -> Size {
        self.size
    }

    /// Draw the sample. This consumes the `SamplePicture`.
    pub fn draw(&self, ctx: &mut T) -> Result<(), Error> {
        (self.draw_f)(ctx)
    }
}

impl Args {
    fn from_env() -> Result<Args, BoxErr> {
        let mut args = pico_args::Arguments::from_env();
        let out_dir: Option<PathBuf> = args.opt_value_from_str("--out")?;

        let args = Args {
            all: args.contains("--all"),
            out_dir: out_dir.unwrap_or_else(|| PathBuf::from(".")),
            number: args.free_from_str()?,
        };

        if !args.all && args.number.is_none() {
            Err(Box::new(Error::InvalidSampleArgs))
        } else {
            Ok(args)
        }
    }
}

/// Run all samples, collecting and printing any errors encountered, without
/// aborting.
///
/// If any errors are encountered, the first is returned on completion.
fn run_all(f: impl Fn(usize) -> Result<(), BoxErr>) -> Result<(), BoxErr> {
    let mut errs = Vec::new();
    for sample in 0..SAMPLE_COUNT {
        if let Err(e) = f(sample) {
            errs.push((sample, e));
        }
    }

    if errs.is_empty() {
        Ok(())
    } else {
        for (sample, err) in &errs {
            eprintln!("error in sample {}: '{}'", sample, err);
        }
        Err(errs.remove(0).1)
    }
}
