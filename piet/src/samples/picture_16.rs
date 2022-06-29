//! capture_image_rect
//!
//! This tests makes sure that copying part of an image works

use crate::kurbo::{Rect, Size};
use crate::{Color, Error, InterpolationMode, RenderContext};

pub const SIZE: Size = Size::new(200., 200.);

const RED: Color = Color::rgb8(255, 0, 0);
const BLUE: Color = Color::rgb8(0, 0, 255);
const INTERPOLATION_MODE: InterpolationMode = InterpolationMode::NearestNeighbor;
const BORDER_WIDTH: f64 = 2.0;

pub fn draw<R: RenderContext>(rc: &mut R) -> Result<(), Error> {
    rc.clear(None, Color::FUCHSIA);

    let outer_rect_red = Rect::new(20., 20., 180., 180.);
    let inner_rect_blue = Rect::new(
        20. + BORDER_WIDTH,
        20. + BORDER_WIDTH,
        180. - BORDER_WIDTH,
        180. - BORDER_WIDTH
    );

    // Draw a box with a red border
    rc.fill(outer_rect_red, &RED);
    rc.fill(inner_rect_blue, &BLUE);

    // Cache the box, clear the image and re-draw the box from the cache
    let cache = rc.capture_image_area(outer_rect_red).unwrap();
    rc.clear(None, Color::BLACK);
    rc.draw_image(&cache, outer_rect_red, INTERPOLATION_MODE);

    // Draw the cached image, scaled, in all four corners of the image
    let top_left_corner = Rect::new(5., 5., 50., 50.);
    let top_right_corner = Rect::new(150., 5., 195., 50.);
    let bottom_left_corner = Rect::new(5., 150., 50., 195.);
    let bottom_right_corner = Rect::new(150., 150., 195., 195.);

    rc.draw_image(&cache, top_left_corner, INTERPOLATION_MODE);
    rc.draw_image(&cache, top_right_corner, INTERPOLATION_MODE);
    rc.draw_image(&cache, bottom_left_corner, INTERPOLATION_MODE);
    rc.draw_image(&cache, bottom_right_corner, INTERPOLATION_MODE);

    Ok(())
}
