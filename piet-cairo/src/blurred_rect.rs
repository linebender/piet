//! Helpers for efficiently drawing blurred rectangles.
//!
//! This function is not cairo-specific, but currently cairo is the only back-end
//! that requires it, as other back-ends have their own implementation.
//!
//! As a future performance optimization, we might implement a cache of the computed
//! images.

use cairo::{Format, ImageSurface};

use piet::kurbo::{Point, Rect};

/// Extent to which to expand the blur.
const BLUR_EXTENT: f64 = 2.5;

pub fn compute_blurred_rect(rect: Rect, radius: f64) -> (ImageSurface, Point) {
    let radius_recip = radius.recip();
    let xmax = rect.width() * radius_recip;
    let ymax = rect.height() * radius_recip;
    let padding = BLUR_EXTENT * radius;
    let rect_padded = rect.inflate(padding, padding);
    let rect_exp = rect_padded.expand();
    let xfrac = rect_padded.x0 - rect_exp.x0;
    let yfrac = rect_padded.y0 - rect_exp.y0;
    let width = rect_exp.width() as usize;
    let height = rect_exp.height() as usize;
    let strip = (0..width)
        .map(|i| {
            let x = ((i as f64) - (xfrac + padding)) * radius_recip;
            (255.0 * 0.25) * (compute_erf7(x) + compute_erf7(xmax - x))
        })
        .collect::<Vec<_>>();
    // TODO: don't panic on error
    let mut image = ImageSurface::create(Format::A8, width as i32, height as i32).unwrap();
    let stride = image.get_stride() as usize;
    {
        let mut data = image.get_data().unwrap();
        for j in 0..height {
            let y = ((j as f64) - (yfrac + padding)) * radius_recip;
            let z = compute_erf7(y) + compute_erf7(ymax - y);
            for i in 0..width {
                data[j * stride + i] = (z * strip[i]).round() as u8;
            }
        }
    }
    let origin = rect_exp.origin();
    (image, origin)
}

// See https://raphlinus.github.io/audio/2018/09/05/sigmoid.html for a little
// explanation of this approximation to the erf function.
fn compute_erf7(x: f64) -> f64 {
    let x = x * std::f64::consts::FRAC_2_SQRT_PI;
    let xx = x * x;
    let x = x + (0.24295 + (0.03395 + 0.0104 * xx) * xx) * (x * xx);
    x / (1.0 + x * x).sqrt()
}
