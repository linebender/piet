//! A wide assortment of graphics meant to show off many different uses of piet

use piet::kurbo::{Affine, BezPath, Line, Point, Rect, RoundedRect, Vec2};

use piet::{
    Color, Error, FontBuilder, ImageFormat, InterpolationMode, RenderContext, Text, TextLayout,
    TextLayoutBuilder,
};

pub fn draw(rc: &mut impl RenderContext) -> Result<(), Error> {
    rc.clear(Color::WHITE);
    let brush = rc.solid_brush(Color::rgb8(0x00, 0x00, 0x80));
    rc.stroke(Line::new((10.0, 10.0), (100.0, 50.0)), &brush, 1.0);

    let mut path = BezPath::new();
    path.move_to((50.0, 10.0));
    path.quad_to((60.0, 50.0), (100.0, 90.0));
    let brush = rc.solid_brush(Color::rgb8(0x00, 0x80, 0x00));
    rc.stroke(path, &brush, 1.0);

    let mut path = BezPath::new();
    path.move_to((10.0, 20.0));
    path.curve_to((10.0, 80.0), (100.0, 80.0), (100.0, 60.0));
    let brush = rc.solid_brush(Color::rgba8(0x00, 0x00, 0x80, 0xC0));
    rc.fill(path, &brush);

    rc.stroke(RoundedRect::new(145.0, 45.0, 185.0, 85.0, 5.0), &brush, 1.0);

    let font = rc.text().new_font_by_name("Segoe UI", 12.0).build()?;
    let layout = rc.text().new_text_layout(&font, "Hello piet!").build()?;
    let w: f64 = layout.width().into();
    let brush = rc.solid_brush(Color::rgba8(0x80, 0x00, 0x00, 0xC0));
    rc.draw_text(&layout, (80.0, 10.0), &brush);

    rc.stroke(Line::new((80.0, 12.0), (80.0 + w, 12.0)), &brush, 1.0);

    rc.with_save(|rc| {
        rc.transform(Affine::rotate(0.1));
        rc.draw_text(&layout, (80.0, 10.0), &brush);
        Ok(())
    })?;

    let image_data = make_image_data(256, 256);
    let image = rc.make_image(256, 256, &image_data, ImageFormat::RgbaSeparate)?;
    rc.draw_image(
        &image,
        Rect::new(150.0, 50.0, 180.0, 80.0),
        InterpolationMode::Bilinear,
    );

    let clip_path = star(Point::new(90.0, 45.0), 10.0, 30.0, 24);
    rc.clip(clip_path);
    let layout = rc.text().new_text_layout(&font, "Clipped text").build()?;
    rc.draw_text(&layout, (80.0, 50.0), &brush);
    Ok(())
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

fn make_image_data(width: usize, height: usize) -> Vec<u8> {
    let mut result = vec![0; width * height * 4];
    for y in 0..height {
        for x in 0..width {
            let ix = (y * width + x) * 4;
            result[ix + 0] = x as u8;
            result[ix + 1] = y as u8;
            result[ix + 2] = !(x as u8);
            result[ix + 3] = 127;
        }
    }
    result
}
