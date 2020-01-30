//! Rendering a cubic Bézier curve with its control points and handles

use piet::kurbo::{BezPath, Line, Point};

use piet::{Color, Error, RenderContext};

// TODO: this will eventually become a `kurbo::Shape`.
fn circle<V: Into<Point>>(center: V, radius: f64, num_segments: usize) -> BezPath {
    let mut path = BezPath::new();
    if num_segments == 0 {
        return path;
    }

    let center = center.into();
    let centerx = center.x;
    let centery = center.y;
    for segment in 0..num_segments {
        let theta = 2.0 * std::f64::consts::PI * (segment as f64) / (num_segments as f64);
        let x = radius * theta.cos();
        let y = radius * theta.sin();
        if segment == 0 {
            path.move_to((x + centerx, y + centery));
        } else {
            let end = (x + centerx, y + centery);
            path.line_to(end);
        }
    }

    path.close_path();
    return path;
}

fn draw_cubic_bezier<V: Into<Point>>(
    rc: &mut impl RenderContext,
    p0: V,
    p1: V,
    p2: V,
    p3: V,
) -> Result<(), Error> {
    let p0 = p0.into();
    let p1 = p1.into();
    let p2 = p2.into();
    let p3 = p3.into();
    let mut path = BezPath::new();
    path.move_to(p0);
    path.curve_to(p1, p2, p3);
    let curve_brush = rc.solid_brush(Color::rgb8(0x00, 0x80, 0x00));
    rc.stroke(&path, &curve_brush, 3.0);

    let handle_brush = rc.solid_brush(Color::rgb8(0x00, 0x00, 0x80));
    rc.stroke(&Line::new(p0, p1), &handle_brush, 1.0);
    rc.stroke(&Line::new(p2, p3), &handle_brush, 1.0);

    for p in [p0, p1, p2, p3].iter() {
        let dot = circle(*p, 1.5, 20);
        rc.fill(&dot, &handle_brush);
    }
    Ok(())
}

pub fn draw<R: RenderContext>(rc: &mut R) -> Result<(), Error> {
    rc.clear(Color::WHITE);
    draw_cubic_bezier(rc, (70.0, 80.0), (140.0, 10.0), (60.0, 10.0), (90.0, 80.0))
}
