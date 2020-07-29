//! Drawing examples for testing backends

use std::collections::BTreeMap;
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
    compare_dir: Option<PathBuf>,
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

    if let Some(compare_dir) = args.compare_dir.as_ref() {
        if let Err(e) = compare_snapshots(compare_dir, &args.out_dir) {
            eprintln!("{}", e);
            std::process::exit(1);
        }
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
            compare_dir: args.opt_value_from_str("--compare")?,
            number: args.free_from_str()?,
        };

        if !(args.all || args.number.is_some() || args.compare_dir.is_some()) {
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

fn compare_snapshots(base: &Path, revised: &Path) -> Result<(), BoxErr> {
    let mut failures = Vec::new();
    let base_paths = get_sample_files(base)?;
    let rev_paths = get_sample_files(revised)?;

    for (number, base_path) in &base_paths {
        let rev_path = match rev_paths.get(number) {
            Some(path) => path,
            None => {
                failures.push(ComparisonError::Missing(*number));
                continue;
            }
        };

        if !compare_files(&base_path, rev_path)? {
            failures.push(ComparisonError::DifferentData(*number));
        }
    }

    for key in rev_paths.keys().filter(|k| !base_paths.contains_key(k)) {
        eprintln!("Example {} exists in revision but not in base", key);
    }

    if failures.is_empty() {
        eprintln!("Compared {} items", base_paths.len());
        Ok(())
    } else {
        Err(Box::new(SnapshotError { failures }))
    }
}

// this can get fancier at some point if we like
fn compare_files(p1: &Path, p2: &Path) -> Result<bool, BoxErr> {
    let one = std::fs::read(p1)?;
    let two = std::fs::read(p2)?;
    Ok(one == two)
}

fn get_sample_files(in_dir: &Path) -> Result<BTreeMap<usize, PathBuf>, BoxErr> {
    let mut out = BTreeMap::new();
    for entry in std::fs::read_dir(in_dir)? {
        let path = entry?.path();
        if let Some(number) = extract_number(&path) {
            out.insert(number, path);
        }
    }
    Ok(out)
}

/// Extract the '12' from a path to a file like 'cairo-test-12'
fn extract_number(path: &Path) -> Option<usize> {
    let stem = path.file_stem()?;
    let stem_str = stem.to_str()?;
    let stripped = stem_str.split('-').last()?;
    stripped.parse().ok()
}

#[derive(Debug, Clone)]
enum ComparisonError {
    Missing(usize),
    DifferentData(usize),
}

#[derive(Debug, Clone)]
struct SnapshotError {
    failures: Vec<ComparisonError>,
}

impl std::fmt::Display for ComparisonError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ComparisonError::Missing(n) => write!(f, "{:>2}: Revision is missing", n),
            ComparisonError::DifferentData(n) => write!(f, "{:>2}: Data differs", n),
        }
    }
}

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f, "Encountered {} failures", self.failures.len())?;
        for failure in &self.failures {
            writeln!(f, "{}", failure)?;
        }
        Ok(())
    }
}

impl std::error::Error for SnapshotError {}
