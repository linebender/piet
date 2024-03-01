// Copyright 2022 the Piet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! capture_image_rect
//!
//! This tests makes sure that copying part of an image works

use crate::kurbo::{Rect, Size};
use crate::{Color, Error, InterpolationMode, RenderContext};

pub const SIZE: Size = Size::new(200., 200.);

const RED: Color = Color::rgb8(255, 0, 0);
const BLUE: Color = Color::rgb8(0, 0, 255);
const INTERPOLATION_MODE: InterpolationMode = InterpolationMode::NearestNeighbor;
const BORDER_WIDTH: f64 = 4.0;

pub fn draw<R: RenderContext>(rc: &mut R) -> Result<(), Error> {
    rc.clear(None, Color::FUCHSIA);

    let outer_rect_red = Rect::new(20., 20., 180., 180.);
    let inner_rect_blue = outer_rect_red.inset(-BORDER_WIDTH);

    // Draw a box with a red border
    rc.fill(outer_rect_red, &RED);
    rc.fill(inner_rect_blue, &BLUE);

    // Cache the box, clear the image and re-draw the box from the cache
    let cache = rc.capture_image_area(outer_rect_red).unwrap();
    rc.clear(None, Color::BLACK);
    rc.draw_image(&cache, outer_rect_red, INTERPOLATION_MODE);

    // Draw the cached image, scaled, in all four corners of the image
    let top_left_corner = Rect::from_origin_size((5., 5.), (40., 40.));
    let top_right_corner = Rect::from_origin_size((155., 5.), (40., 40.));
    let bottom_left_corner = Rect::from_origin_size((5., 155.), (40., 40.));
    let bottom_right_corner = Rect::from_origin_size((155., 155.), (40., 40.));

    rc.draw_image(&cache, top_left_corner, INTERPOLATION_MODE);
    rc.draw_image(&cache, top_right_corner, INTERPOLATION_MODE);
    rc.draw_image(&cache, bottom_left_corner, INTERPOLATION_MODE);
    rc.draw_image(&cache, bottom_right_corner, INTERPOLATION_MODE);

    Ok(())
}
