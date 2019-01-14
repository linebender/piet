//! Test code for piet.

// Right now, this is just code to generate sample images.

use piet::RenderContext;
mod picture_0;
mod picture_1;

use crate::picture_0::draw as draw_picture_0;
use crate::picture_1::draw as draw_picture_1;

/// Draw a test picture, by number.
///
/// Hopefully there will be a suite of test pictures. For now, there is just the one.
pub fn draw_test_picture(rc: &mut impl RenderContext, number: usize) {
    match number {
        0 => draw_picture_0(rc),
        1 => draw_picture_1(rc),
        _ => eprintln!(
            "Don't have test picture {} yet. Why don't you make it?",
            number
        ),
    }
}
