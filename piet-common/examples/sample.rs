//! Draw a sample image with this backend.

// TODO: Remove all the wasm32 cfg guards once this compiles with piet-web
//
#[cfg(all(feature = "png", not(target_arch = "wasm32")))]
fn main() {
    use piet::RenderContext;
    use piet_common::Device;

    const WIDTH: usize = 400;
    const HEIGHT: usize = 200;
    const HIDPI: f64 = 2.0;

    let test_picture_number = std::env::args()
        .nth(1)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
    let mut device = Device::new().unwrap();
    let mut bitmap = device.bitmap_target(WIDTH, HEIGHT, HIDPI).unwrap();
    let mut rc = bitmap.render_context();
    piet::draw_test_picture(&mut rc, test_picture_number).unwrap();
    rc.finish().unwrap();
    std::mem::drop(rc);

    let path = format!(
        "{}-sample-{}.png",
        piet_common::BACKEND_NAME,
        test_picture_number
    );

    bitmap.save_to_file(&path).expect("file save error");
}

#[cfg(any(not(feature = "png"), target_arch = "wasm32"))]
fn main() {
    panic!("This example requires the 'png' feature, and does not run on wasm32")
}
