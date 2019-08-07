//! Rendering stroke styles.

use piet::kurbo::{Affine, BezPath, Line};

use piet::{Color, Error, LineCap, LineJoin, RenderContext, StrokeStyle};

pub fn draw<R: RenderContext>(rc: &mut R) -> Result<(), Error> {
    rc.clear(Color::WHITE);

    let mut path = BezPath::new();
    path.move_to((0.0, 0.0));
    path.line_to((20.0, 0.0));
    path.line_to((6.0, 10.0));
    let mut y = 5.0;
    let brush = rc.solid_brush(Color::rgb8(0x00, 0x00, 0xC0));
    for line_cap in &[LineCap::Butt, LineCap::Round, LineCap::Square] {
        let mut x = 5.0;
        for line_join in &[LineJoin::Bevel, LineJoin::Miter, LineJoin::Round] {
            let width = 5.0;
            let mut style = StrokeStyle::new();
            rc.with_save(|rc| {
                rc.transform(Affine::translate((x, y)));
                style.set_line_cap(*line_cap);
                style.set_line_join(*line_join);
                rc.stroke_styled(&path, &brush, width, &style);
                Ok(())
            })?;
            x += 30.0;
        }
        y += 30.0;
    }

    y = 5.0;
    let x = 100.0;
    let mut dashes = Vec::new();
    for i in 0..8 {
        let mut style = StrokeStyle::new();
        dashes.push((i + 1) as f64);
        style.set_dash(dashes.clone(), 0.0);
        rc.stroke_styled(Line::new((x, y), (x + 50.0, y)), &brush, 2.0, &style);
        y += 10.0;
    }
    Ok(())
}
