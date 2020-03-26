// TODO: Remove all the wasm32 cfg guards once this compiles with piet-web

#[cfg(not(target_arch = "wasm32"))]
use piet::kurbo::Line;
#[cfg(not(target_arch = "wasm32"))]
use piet::{Color, RenderContext};
#[cfg(not(target_arch = "wasm32"))]
use piet_common::Device;

/// Feature "png" needed for save_to_file() and it's disabled by default for optionsl dependencies
/// cargo run --example png --features png
fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
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
}
