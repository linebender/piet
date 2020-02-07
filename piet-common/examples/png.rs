use piet::kurbo::Line;
use piet::{Color, RenderContext};
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

    bitmap
        .save_to_file("temp-image.png")
        .expect("file save error");
}
