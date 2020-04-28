use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use core_graphics::color_space::CGColorSpace;
use core_graphics::context::CGContext;

use piet::kurbo::Circle;
use piet::{Color, RenderContext};

const WIDTH: usize = 800;
const HEIGHT: usize = 600;

fn main() {
    let mut cg_ctx = CGContext::create_bitmap_context(
        None,
        WIDTH,
        HEIGHT,
        8,
        0,
        &CGColorSpace::create_device_rgb(),
        core_graphics::base::kCGImageAlphaPremultipliedLast,
    );
    let mut piet = piet_coregraphics::CoreGraphicsContext::new(&mut cg_ctx);
    piet.fill(
        Circle::new((100.0, 100.0), 50.0),
        &Color::rgb8(255, 0, 0).with_alpha(0.5),
    );
    piet.finish().unwrap();

    unpremultiply(cg_ctx.data());

    // Write image as PNG file.
    let path = Path::new("image.png");
    let file = File::create(path).unwrap();
    let ref mut w = BufWriter::new(file);

    let mut encoder = png::Encoder::new(w, WIDTH as u32, HEIGHT as u32);
    encoder.set_color(png::ColorType::RGBA);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();

    writer.write_image_data(cg_ctx.data()).unwrap();
}

fn unpremultiply(data: &mut [u8]) {
    for i in (0..data.len()).step_by(4) {
        let a = data[i + 3];
        if a != 0 {
            let scale = 255.0 / (a as f64);
            data[i] = (scale * (data[i] as f64)).round() as u8;
            data[i + 1] = (scale * (data[i + 1] as f64)).round() as u8;
            data[i + 2] = (scale * (data[i + 2] as f64)).round() as u8;
        }
    }
}
