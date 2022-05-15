//! Clipping and clearing.
//!
//! This tests interactions between clipping, transforms, and the clear method.
//!
//! 1. clear ignores clipping and transforms

use crate::kurbo::{Affine, Circle, Rect, Size};
use crate::{Color, Error, InterpolationMode, RenderContext};

pub const SIZE: Size = Size::new(400., 400.);

const RED: Color = Color::rgb8(255, 0, 0);
const BLUE: Color = Color::rgb8(0, 0, 255);
const SEMI_TRANSPARENT_GREEN: Color = Color::rgba8(0, 255, 0, 125);
const SEMI_TRANSPARENT_WHITE: Color = Color::rgba8(255, 255, 255, 125);

pub fn draw<R: RenderContext>(rc: &mut R) -> Result<(), Error> {
    rc.clear(None, Color::BLACK);

    let outer_rect = Rect::new(20., 20., 180., 180.);
    let inner_rect = Rect::new(21., 21., 179., 179.);

    let top_left_corner = Rect::new(5., 5., 50., 50.);
    let top_right_corner = Rect::new(150., 5., 195., 50.);
    let bottom_left_corner = Rect::new(5., 150., 50., 195.);
    let bottom_right_corner = Rect::new(150., 150., 195., 195.);


    // Draw a box with a red border
    rc.fill(outer_rect, &RED);
    rc.fill(inner_rect, &BLUE);

    // Cache the box, clear the image and re-draw the box from the cache
    let cache = rc.capture_image_area(outer_rect).unwrap();
    rc.clear(None, Color::WHITE);
    rc.draw_image(&cache, outer_rect, InterpolationMode::NearestNeighbor);

    // Draw the cached image, scaled, in all four corners of the image
    rc.draw_image(&cache, top_left_corner, InterpolationMode::Bilinear);
    rc.draw_image(&cache, top_right_corner, InterpolationMode::Bilinear);
    rc.draw_image(&cache, bottom_left_corner, InterpolationMode::Bilinear);
    rc.draw_image(&cache, bottom_right_corner, InterpolationMode::Bilinear);

    Ok(())
}
