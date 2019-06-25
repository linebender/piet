//! A bunch of image test cases.

use piet::kurbo::Rect;
use piet::{Color, Error, ImageFormat, InterpolationMode, RenderContext};

pub fn draw(rc: &mut impl RenderContext) -> Result<(), Error> {
    rc.clear(Color::WHITE);

    let mut y = 5.0;
    for &mode in &[
        InterpolationMode::NearestNeighbor,
        InterpolationMode::Bilinear,
    ] {
        let mut x = 5.0;
        for &format in &[
            ImageFormat::RgbaSeparate,
            ImageFormat::RgbaPremul,
            ImageFormat::Rgb,
        ] {
            let image_data = make_image_data(16, 16, format);
            let image = rc.make_image(16, 16, &image_data, format)?;
            rc.draw_image(&image, Rect::new(x, y, x + 40.0, y + 40.0), mode);
            x += 50.0;
        }
        y += 50.0;
    }
    Ok(())
}

fn make_image_data(width: usize, height: usize, format: ImageFormat) -> Vec<u8> {
    let bytes_per_pixel = format.bytes_per_pixel();
    let mut result = vec![0; width * height * bytes_per_pixel];
    for y in 0..height {
        for x in 0..width {
            let ix = (y * width + x) * bytes_per_pixel;
            let r = (x * 255 / (width - 1)) as u8;
            let g = (y * 255 / (height - 1)) as u8;
            let b = !r;
            let r2 = ((x as f64) - 8.0).powi(2) + ((y as f64) - 8.0).powi(2);
            let a = (255.0 * (-0.01 * r2).exp()) as u8;
            match format {
                ImageFormat::RgbaSeparate => {
                    result[ix + 0] = r;
                    result[ix + 1] = g;
                    result[ix + 2] = b;
                    result[ix + 3] = a;
                }
                ImageFormat::RgbaPremul => {
                    fn premul(x: u8, a: u8) -> u8 {
                        let y = (x as u16) * (a as u16);
                        ((y + (y >> 8) + 0x80) >> 8) as u8
                    }
                    result[ix + 0] = premul(r, a);
                    result[ix + 1] = premul(g, a);
                    result[ix + 2] = premul(b, a);
                    result[ix + 3] = a;
                }
                ImageFormat::Rgb => {
                    result[ix + 0] = r;
                    result[ix + 1] = g;
                    result[ix + 2] = b;
                }
                _ => (),
            }
        }
    }
    result
}
