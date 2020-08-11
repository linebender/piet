//! Drawing examples for testing backends

use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufWriter;
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
mod picture_12;
mod picture_13;
mod picture_14;

type BoxErr = Box<dyn std::error::Error>;

/// The total number of samples in this module.
pub const SAMPLE_COUNT: usize = 15;

/// file we save an os fingerprint to
pub const GENERATED_BY: &str = "GENERATED_BY";

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
        12 => SamplePicture::new(picture_12::SIZE, picture_12::draw),
        13 => SamplePicture::new(picture_13::SIZE, picture_13::draw),
        14 => SamplePicture::new(picture_14::SIZE, picture_14::draw),
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
///
/// The `prefix` argument is used for the file names of failure cases.
pub fn samples_main(f: fn(usize, &Path) -> Result<(), BoxErr>, prefix: &str) -> Result<(), BoxErr> {
    let args = Args::from_env()?;

    if !args.out_dir.exists() {
        std::fs::create_dir_all(&args.out_dir)?;
    }

    if args.all {
        write_os_info(&args.out_dir)?;
        run_all(|number| f(number, &args.out_dir))?;
    } else if let Some(number) = args.number {
        f(number, &args.out_dir)?;
    }

    if let Some(compare_dir) = args.compare_dir.as_ref() {
        let info_one = read_os_info(compare_dir)?;
        let info_two = read_os_info(&args.out_dir)?;
        let results = compare_snapshots(compare_dir, &args.out_dir, prefix)?;
        println!("Compared {} snapshots", results.len());
        print!("base: {}", info_one);
        println!("rev : {}", info_two);

        for (number, result) in results.iter() {
            print!("Image {:02}: ", number);
            match result {
                Some(failure) => println!("{}", failure),
                None => println!("Ok"),
            }
        }

        let exit_code = if results.values().any(Option::is_some) {
            1
        } else {
            0
        };
        std::process::exit(exit_code);
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

fn compare_snapshots(
    base: &Path,
    revised: &Path,
    prefix: &str,
) -> Result<BTreeMap<usize, Option<FailureReason>>, BoxErr> {
    let mut failures = BTreeMap::new();
    let base_paths = get_sample_files(base)?;
    let rev_paths = get_sample_files(revised)?;

    for (number, base_path) in &base_paths {
        let rev_path = match rev_paths.get(number) {
            Some(path) => path,
            None => {
                failures.insert(*number, Some(FailureReason::MissingRevision));
                continue;
            }
        };

        let result = compare_files(*number, &base_path, rev_path, prefix)?;
        failures.insert(*number, result);
    }

    for key in rev_paths.keys().filter(|k| !base_paths.contains_key(k)) {
        failures.insert(*key, Some(FailureReason::MissingBase));
    }
    Ok(failures)
}

// this can get fancier at some point if we like
fn compare_files(
    number: usize,
    p1: &Path,
    p2: &Path,
    prefix: &str,
) -> Result<Option<FailureReason>, BoxErr> {
    let (one_info, one) = get_png_data(p1)?;
    let (two_info, two) = get_png_data(p2)?;
    let one_size = Size::new(one_info.width as f64, one_info.height as f64);
    let two_size = Size::new(two_info.width as f64, two_info.height as f64);
    if one_size != two_size {
        return Ok(Some(FailureReason::WrongSize {
            base: one_size,
            rev: two_size,
        }));
    }
    assert_eq!(
        one_info.color_type, two_info.color_type,
        "color types should always match"
    );
    let err_write_path = p2.with_file_name(&format!("{}{}-diff.png", prefix, number));
    compare_pngs(one_info, &one, &two, err_write_path)
}

fn get_png_data(path: &Path) -> Result<(png::OutputInfo, Vec<u8>), BoxErr> {
    let decoder = png::Decoder::new(File::open(path)?);
    let (info, mut reader) = decoder.read_info()?;
    // Allocate the output buffer.
    let mut buf = vec![0; info.buffer_size()];
    // Read the next frame. An APNG might contain multiple frames.
    reader.next_frame(&mut buf)?;
    Ok((info, buf))
}

/// Compare two pngs; in the case of difference, write a visualization of that difference
/// to `write_path`.
///
/// Returns `Err` if there is an intermediate error; returns `Ok(None)` if the pngs
/// are identical, and `Ok(Some(PathBuf))` if they are different, returning the path
/// we were given.
fn compare_pngs(
    info: png::OutputInfo,
    one: &[u8],
    two: &[u8],
    write_path: PathBuf,
) -> Result<Option<FailureReason>, BoxErr> {
    if one == two {
        return Ok(None);
    }
    let samples = info.color_type.samples();
    assert_eq!(one.len(), two.len(), "buffers must have equal length");
    assert_eq!(
        one.len() % samples,
        0,
        "png buffer length should be divisible by number of samples"
    );

    let file = File::create(&write_path)?;
    let mut w = BufWriter::new(file);

    let mut encoder = png::Encoder::new(&mut w, info.width, info.height); // Width is 2 pixels and height is 1.
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;

    let mut buf = vec![0; (info.width * info.height) as usize];

    let mut overall_diff = 0.;
    for (i, (p1, p2)) in one.chunks(samples).zip(two.chunks(samples)).enumerate() {
        let total_diff: i32 = p1
            .iter()
            .zip(p2.iter())
            .map(|(one, two)| (*one as i32 - *two as i32).abs())
            .sum();
        let avg_diff = total_diff / samples as i32;
        overall_diff += total_diff as f32 / samples as f32;
        let avg_diff = if avg_diff > 0 {
            // we want all difference to be visible, so we set a threshold.
            avg_diff.max(24) as u8
        } else {
            0
        };
        buf[i] = avg_diff;
    }

    let overall_avg = overall_diff / buf.len() as f32;
    let avg_perc = (overall_avg / 0xFF as f32) * 100.;

    writer.write_image_data(&buf)?;
    Ok(Some(FailureReason::DifferentData {
        avg_diff_pct: avg_perc,
        diff_path: write_path,
    }))
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

fn write_os_info(base_dir: &Path) -> std::io::Result<()> {
    let path = base_dir.join(GENERATED_BY);
    std::fs::write(&path, make_os_info_string().as_bytes())
}

fn read_os_info(base_dir: &Path) -> std::io::Result<String> {
    let path = base_dir.join(GENERATED_BY);
    std::fs::read_to_string(&path)
}

/// Get info about the system used to create these samples.
//TODO: include info about generic fonts? anything else?
fn make_os_info_string() -> String {
    let info = os_info::get();
    format!("{} {}\n", info.os_type(), info.version())
}

#[derive(Debug, Clone)]
enum FailureReason {
    MissingBase,
    MissingRevision,
    WrongSize {
        base: Size,
        rev: Size,
    },
    DifferentData {
        avg_diff_pct: f32,
        diff_path: PathBuf,
    },
}

#[derive(Debug, Clone)]
struct ComparisonError {
    number: usize,
    reason: FailureReason,
}

#[derive(Debug, Clone)]
struct SnapshotError {
    failures: Vec<ComparisonError>,
}

impl std::fmt::Display for FailureReason {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            FailureReason::MissingBase => write!(f, "Base file is missing"),
            FailureReason::MissingRevision => write!(f, "Revised file is missing"),
            FailureReason::DifferentData {
                avg_diff_pct,
                diff_path,
            } => write!(
                f,
                "Data differs {:>5.2}%: {}",
                avg_diff_pct,
                diff_path.to_string_lossy(),
            ),
            FailureReason::WrongSize { base, rev } => {
                write!(f, "Mismatched sizes, base {}, revision {}", base, rev)
            }
        }
    }
}
