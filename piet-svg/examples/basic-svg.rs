//! Basic example of rendering to a SVG

use std::io;

use piet::draw_test_picture;
use piet::RenderContext;

fn main() {
    let test_picture_number = std::env::args()
        .nth(1)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
    let mut piet = piet_svg::RenderContext::new();
    draw_test_picture(&mut piet, test_picture_number).unwrap();
    piet.finish().unwrap();
    piet.write(io::stdout()).unwrap();
}
