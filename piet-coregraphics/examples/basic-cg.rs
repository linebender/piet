use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use core_graphics::color_space::CGColorSpace;
use core_graphics::context::CGContext;

use piet::kurbo::{Circle, Rect, Size};
use piet::{
    Color, FontBuilder, LinearGradient, RadialGradient, RenderContext, Text, TextLayout,
    TextLayoutBuilder, UnitPoint,
};

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
    let mut piet = piet_coregraphics::CoreGraphicsContext::new_y_up(&mut cg_ctx, HEIGHT as f64);
    let bounds = Size::new(WIDTH as f64, HEIGHT as f64).to_rect();

    let linear = LinearGradient::new(
        UnitPoint::TOP_LEFT,
        UnitPoint::BOTTOM_RIGHT,
        (
            Color::rgba(1.0, 0.2, 0.5, 0.4),
            Color::rgba(0.9, 0.0, 0.9, 0.8),
        ),
    );
    let radial = RadialGradient::new(0.8, (Color::WHITE, Color::BLACK))
        .with_origin(UnitPoint::new(0.2, 0.7));

    piet.fill(bounds.inset((0., 0., -bounds.width() * 0.5, 0.)), &radial);
    piet.fill(Circle::new((100.0, 100.0), 50.0), &linear);
    piet.stroke(bounds, &linear, 20.0);

    let font = piet
        .text()
        .new_font_by_name("Georgia", 24.0)
        .build()
        .unwrap();

    let mut layout = piet
        .text()
        .new_text_layout(&font, "this is my cool\nmultiline string, I like it very much, do you also like it? why or why not? Show your work.", None)
        .build()
        .unwrap();

    piet.blurred_rect(Rect::new(100.0, 100., 150., 150.), 5.0, &Color::BLACK);
    piet.fill(
        Rect::new(95.0, 105., 145., 155.),
        &Color::rgb(0.0, 0.4, 0.2),
    );
    piet.draw_text(&layout, (0., 00.0), &Color::WHITE);
    layout.update_width(400.).unwrap();
    piet.draw_text(&layout, (200.0, 200.0), &Color::BLACK);
    layout.update_width(200.).unwrap();
    piet.draw_text(&layout, (400.0, 400.0), &Color::rgba8(255, 0, 0, 150));

    piet.finish().unwrap();
    std::mem::drop(piet);

    unpremultiply(cg_ctx.data());

    // Write image as PNG file.
    let path = Path::new("image.png");
    let file = File::create(path).unwrap();
    let w = BufWriter::new(file);

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
