use std::sync::Arc;

use core_graphics::{color_space::CGColorSpace, data_provider::CGDataProvider, image::CGImage};
use piet::kurbo::Rect;

/// Extent to which to expand the blur.
const BLUR_EXTENT: f64 = 2.5;

//TODO: reuse between this and cairo

pub(crate) fn compute_blurred_rect(rect: Rect, radius: f64) -> (CGImage, Rect) {
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

    let mut data = vec![0u8; width * height];

    for j in 0..height {
        let y = ((j as f64) - (yfrac + padding)) * radius_recip;
        let z = compute_erf7(y) + compute_erf7(ymax - y);
        for i in 0..width {
            data[j * width + i] = (z * strip[i]).round() as u8;
        }
    }

    let data_provider = CGDataProvider::from_buffer(Arc::new(data));
    let color_space = CGColorSpace::create_device_gray();
    let image = CGImage::new(
        width,
        height,
        8,
        8,
        width,
        &color_space,
        0,
        &data_provider,
        false,
        0,
    );
    (image, rect_exp)
}

// See https://raphlinus.github.io/audio/2018/09/05/sigmoid.html for a little
// explanation of this approximation to the erf function.
fn compute_erf7(x: f64) -> f64 {
    let x = x * std::f64::consts::FRAC_2_SQRT_PI;
    let xx = x * x;
    let x = x + (0.24295 + (0.03395 + 0.0104 * xx) * xx) * (x * xx);
    x / (1.0 + x * x).sqrt()
}
