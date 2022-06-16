//! Basic example of rendering to a SVG

use std::io;

use piet::RenderContext;
use piet_test::samples;

fn main() {
    let test_picture_number = std::env::args()
        .nth(1)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
    let sample = samples::get(test_picture_number).unwrap();
    let mut piet = piet_svg::RenderContext::new(sample.size());
    sample.draw(&mut piet).unwrap();
    piet.finish().unwrap();
    piet.write(io::stdout()).unwrap();
}
