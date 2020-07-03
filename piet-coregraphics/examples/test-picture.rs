//! Run the piet-test examples with the coregraphics backend.

use std::fs::File;
use std::io::BufWriter;

use core_graphics::color_space::CGColorSpace;
use core_graphics::context::CGContext;

use piet::RenderContext;
use piet_coregraphics::CoreGraphicsContext;

const SCALE: f64 = 2.0;

fn main() {
    let test_picture_number = std::env::args()
        .nth(1)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);

    let size = piet::size_for_test_picture(test_picture_number).unwrap();
    let mut cg_ctx = CGContext::create_bitmap_context(
        None,
        size.width as usize,
        size.height as usize,
        8,
        0,
        &CGColorSpace::create_device_rgb(),
        core_graphics::base::kCGImageAlphaPremultipliedLast,
    );
    cg_ctx.scale(SCALE, SCALE);
    let mut piet_context = CoreGraphicsContext::new_y_up(&mut cg_ctx, size.height * SCALE.recip());
    piet::draw_test_picture(&mut piet_context, test_picture_number).unwrap();
    piet_context.finish().unwrap();
    std::mem::drop(piet_context);
    let file = File::create(format!("coregraphics-test-{}.png", test_picture_number)).unwrap();
    let w = BufWriter::new(file);

    let mut encoder = png::Encoder::new(w, size.width as u32, size.height as u32);
    encoder.set_color(png::ColorType::RGBA);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();

    piet_coregraphics::unpremultiply_rgba(cg_ctx.data());
    writer.write_image_data(cg_ctx.data()).unwrap();
}
