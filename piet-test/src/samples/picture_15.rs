//! Clipping and clearing.
//!
//! This tests interactions between clipping, transforms, and the clear method.
//!
//! 1. clear ignores clipping and transforms

use piet::kurbo::{Affine, Circle, Rect, Size};
use piet::{Color, Error, RenderContext};

pub const SIZE: Size = Size::new(400., 400.);

const RED: Color = Color::rgb8(255, 0, 0);
const BLUE: Color = Color::rgb8(0, 0, 255);
const SEMI_TRANSPARENT_GREEN: Color = Color::rgba8(0, 255, 0, 125);
const SEMI_TRANSPARENT_WHITE: Color = Color::rgba8(255, 255, 255, 125);

pub fn draw<R: RenderContext>(rc: &mut R) -> Result<(), Error> {
    rc.clear(None, Color::BLACK);
    let circle = Circle::new((100., 100.), 90.);
    let rect = Rect::new(20., 20., 180., 180.);
    let degrees = 45.0;
    let radians = degrees * std::f64::consts::PI / 180.;

    let rotate = Affine::translate((100., 100.))
        * Affine::rotate(radians)
        * Affine::translate((-100., -100.0));

    rc.save().unwrap();
    rc.clip(circle);
    rc.transform(rotate);

    // this should ignore the translation and the clipping, and clear everything
    rc.clear(None, SEMI_TRANSPARENT_WHITE);

    // this should ignore the translate and clipping and clear just the rect
    rc.clear(rect, BLUE);

    // this should respect the existing transform, and be rotated and clipped
    rc.fill(rect, &RED);
    rc.restore().unwrap();

    // this should not be clipped at all
    let left_circ = Circle::new((10., 100.), 10.);
    rc.fill(left_circ, &SEMI_TRANSPARENT_GREEN);

    rc.clip(circle);
    let right_circ = Circle::new((190., 100.), 10.);
    rc.fill(right_circ, &SEMI_TRANSPARENT_GREEN);

    // nested clip: drawing after we clip this should be in the union of the two
    // clip regions:
    let bottom_right_rect = Rect::from_origin_size((150., 150.), (50., 50.));
    rc.clip(bottom_right_rect);

    // this should ignore that clip, and fully clear the bottom left
    let bottom_left_rect = Rect::from_origin_size((0., 150.), (50., 50.));
    rc.clear(bottom_left_rect, SEMI_TRANSPARENT_GREEN);

    // this is filling the whole canvas, but it should be clipped to the union
    // of `circle` and `bottom_right_rect`.
    rc.fill(Rect::new(0., 0., 200., 200.), &SEMI_TRANSPARENT_GREEN);

    Ok(())
}
