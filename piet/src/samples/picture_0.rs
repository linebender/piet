//! A wide assortment of graphics meant to show off many different uses of piet

use crate::kurbo::{Affine, BezPath, Line, Point, Rect, RoundedRect, Size, Vec2};
use crate::{
    Color, Error, FontFamily, ImageFormat, InterpolationMode, RenderContext, Text, TextAttribute,
    TextLayout, TextLayoutBuilder,
};

const BLUE: Color = Color::rgb8(0x00, 0x00, 0x80);
const GREEN: Color = Color::rgb8(0x00, 0x80, 0x00);
const BLUE_ALPHA: Color = Color::rgba8(0x00, 0x00, 0x80, 0xC0);
const RED_ALPHA: Color = Color::rgba8(0x80, 0x00, 0x00, 0xC0);
const YELLOW_ALPHA: Color = Color::rgba8(0xCF, 0xCF, 0x00, 0x60);

pub const SIZE: Size = Size::new(400., 200.);

pub fn draw(rc: &mut impl RenderContext) -> Result<(), Error> {
    rc.clear(None, Color::WHITE);
    rc.stroke(Line::new((10.0, 10.0), (100.0, 50.0)), &BLUE, 1.0);

    let georgia = rc.text().font_family("Georgia").ok_or(Error::MissingFont)?;

    let path = arc1();
    rc.stroke(path, &GREEN, 1.0);

    let path = arc2();
    rc.fill(path, &BLUE_ALPHA);

    rc.stroke(
        RoundedRect::new(145.0, 45.0, 185.0, 85.0, 5.0),
        &BLUE_ALPHA,
        1.0,
    );

    let layout = rc
        .text()
        .new_text_layout("Hello piet!")
        .font(FontFamily::SYSTEM_UI, 12.0)
        .default_attribute(TextAttribute::TextColor(RED_ALPHA))
        .build()?;

    let w: f64 = layout.size().width;
    rc.draw_text(&layout, (80.0, 10.0));

    rc.stroke(Line::new((80.0, 12.0), (80.0 + w, 12.0)), &RED_ALPHA, 1.0);

    rc.with_save(|rc| {
        rc.transform(Affine::rotate(0.1));
        rc.draw_text(&layout, (80.0, 10.0));
        Ok(())
    })?;

    rc.blurred_rect(Rect::new(155.0, 55.0, 185.0, 85.0), 5.0, &Color::BLACK);

    let image_data = make_image_data(256, 256);
    let image = rc.make_image(256, 256, &image_data, ImageFormat::RgbaSeparate)?;
    rc.draw_image(
        &image,
        Rect::new(150.0, 50.0, 180.0, 80.0),
        InterpolationMode::Bilinear,
    );

    // 3x3 px red image with a single blue pixel in the middle
    #[rustfmt::skip]
    let blue_dot_data = [
        255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
        255, 0, 0, 255, 0, 0, 255, 255, 255, 0, 0, 255,
        255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
    ];
    let blue_dot_image = rc.make_image(3, 3, &blue_dot_data, ImageFormat::RgbaPremul)?;
    // Draw using only the single blue pixel
    rc.draw_image_area(
        &blue_dot_image,
        Rect::new(1.0, 1.0, 2.0, 2.0),
        Rect::new(160.0, 20.0, 170.0, 30.0),
        InterpolationMode::NearestNeighbor,
    );

    let clip_path = star(Point::new(90.0, 45.0), 10.0, 30.0, 24);
    rc.fill(&clip_path, &YELLOW_ALPHA);
    rc.clip(clip_path);

    let layout = rc
        .text()
        .new_text_layout("CLIPPED")
        .font(georgia, 8.0)
        .default_attribute(TextAttribute::TextColor(RED_ALPHA))
        .build()?;
    rc.draw_text(&layout, (80.0, 50.0));

    Ok(())
}

fn arc1() -> BezPath {
    let mut path = BezPath::new();
    path.move_to((50.0, 10.0));
    path.quad_to((60.0, 50.0), (100.0, 90.0));
    path
}

fn arc2() -> BezPath {
    let mut path = BezPath::new();
    path.move_to((10.0, 20.0));
    path.curve_to((10.0, 80.0), (100.0, 80.0), (100.0, 60.0));
    path
}

// Note: this could be a Shape.
fn star(center: Point, inner: f64, outer: f64, n: usize) -> BezPath {
    let mut result = BezPath::new();
    let d_th = std::f64::consts::PI / (n as f64);
    for i in 0..n {
        let outer_pt = center + outer * Vec2::from_angle(d_th * ((i * 2) as f64));
        if i == 0 {
            result.move_to(outer_pt);
        } else {
            result.line_to(outer_pt);
        }
        result.line_to(center + inner * Vec2::from_angle(d_th * ((i * 2 + 1) as f64)));
    }
    result.close_path();
    result
}

// allows for nice vertical formatting for `result[ix + 0]`
#[allow(clippy::identity_op)]
fn make_image_data(width: usize, height: usize) -> Vec<u8> {
    let mut result = vec![0; width * height * 4];
    for y in 0..height {
        let in_top = (y / (height / 2)) == 0;
        for x in 0..width {
            let in_left = (x / (width / 2)) == 0;
            let ix = (y * width + x) * 4;
            // I love branching,so sue me
            let (r, g, b) = match (in_top, in_left) {
                (true, true) => (0xff, 0x00, 0x00),
                (true, false) => (0x00, 0xff, 0x00),
                (false, true) => (0x00, 0x00, 0xff),
                (false, false) => (0x00, 0x00, 0x00),
            };
            result[ix + 0] = r;
            result[ix + 1] = g;
            result[ix + 2] = b;
            result[ix + 3] = 127;
        }
    }
    result
}
