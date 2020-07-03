//! Drawing examples for testing backends

use crate::kurbo::Size;
use crate::{Error, RenderContext};

mod picture_0;
mod picture_1;
mod picture_2;
mod picture_3;
mod picture_4;
mod picture_5;
mod picture_6;
mod picture_7;

use picture_0::draw as draw_picture_0;
use picture_1::draw as draw_picture_1;
use picture_2::draw as draw_picture_2;
use picture_3::draw as draw_picture_3;
use picture_4::draw as draw_picture_4;
use picture_5::draw as draw_picture_5;
use picture_6::draw as draw_picture_6;
use picture_7::draw as draw_picture_7;

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
        6 => draw_picture_6(rc),
        7 => draw_picture_7(rc),
        _ => {
            eprintln!(
                "Don't have test picture {} yet. Why don't you make it?",
                number
            );
            Err(Error::InvalidInput)
        }
    }
}

pub fn size_for_test_picture(number: usize) -> Result<Size, Error> {
    match number {
        0 => Ok(picture_0::SIZE),
        1 => Ok(picture_1::SIZE),
        2 => Ok(picture_2::SIZE),
        3 => Ok(picture_3::SIZE),
        4 => Ok(picture_4::SIZE),
        5 => Ok(picture_5::SIZE),
        6 => Ok(picture_6::SIZE),
        7 => Ok(picture_7::SIZE),
        other => {
            eprintln!("test picture {} does not exist.", other);
            Err(Error::InvalidInput)
        }
    }
}
