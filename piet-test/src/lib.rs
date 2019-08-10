//! Test code for piet.

// Right now, this is just code to generate sample images.

use piet::{Error, RenderContext};
mod picture_0;
mod picture_1;
mod picture_2;
mod picture_3;
mod picture_4;
mod picture_5;

use crate::picture_0::draw as draw_picture_0;
use crate::picture_1::draw as draw_picture_1;
use crate::picture_2::draw as draw_picture_2;
use crate::picture_3::draw as draw_picture_3;
use crate::picture_4::draw as draw_picture_4;
use crate::picture_5::draw as draw_picture_5;

/// Draw a test picture, by number.
///
/// There are a few test pictures here now, and hopefully it will grow into
/// a full suite, suitable for both benchmarking and correctness testing.
pub fn draw_test_picture(rc: &mut impl RenderContext, number: usize) -> Result<(), Error> {
    match number {
        0 => draw_picture_0(rc),
        1 => draw_picture_1(rc),
        2 => draw_picture_2(rc),
        3 => draw_picture_3(rc),
        4 => draw_picture_4(rc),
        5 => draw_picture_5(rc),
        _ => {
            eprintln!(
                "Don't have test picture {} yet. Why don't you make it?",
                number
            );
            // TODO: error?
            Ok(())
        }
    }
}
