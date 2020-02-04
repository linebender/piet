use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use png::{ColorType, Encoder, Info, StreamWriter, Writer};

use piet::{Color, ImageFormat, RenderContext};
use piet::kurbo::Line;
use piet_common::Device;

fn main() {
    let mut device = Device::new().unwrap();
    let width = 640;
    let height = 480;
    let mut bitmap = device.bitmap_target(width, height, 1.0).unwrap();
    let mut rc = bitmap.render_context();
    rc.clear(Color::WHITE);
    let brush = rc.solid_brush(Color::rgb8(0x00, 0x00, 0x80));
    rc.stroke(Line::new((10.0, 10.0), (100.0, 50.0)), &brush, 1.0);
    rc.finish().unwrap();
    let raw_pixels = bitmap.into_raw_pixels(ImageFormat::RgbaPremul).unwrap();

    let file = BufWriter::new(File::create(Path::new("temp-image.png")).unwrap());
    let mut encoder = Encoder::new(
        file,
        width as u32,
        height as u32,
    );
    encoder.set_color(ColorType::RGBA);
    encoder.write_header().unwrap()
        .write_image_data(&raw_pixels).unwrap();
}
