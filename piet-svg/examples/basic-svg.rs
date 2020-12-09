//! Basic example of rendering to a SVG

use std::io;

use piet::{samples, RenderContext};

fn main() {
    let test_picture_number = std::env::args()
        .nth(1)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
    let mut piet = piet_svg::RenderContext::new();
    samples::get(test_picture_number)
        .unwrap()
        .draw(&mut piet)
        .unwrap();
    piet.finish().unwrap();
    piet.write(io::stdout()).unwrap();
}
