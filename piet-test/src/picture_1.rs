//! Rendering a cubic Bézier curve with its control points and handles

use piet::kurbo::{BezPath, Line, Vec2};

use piet::{Error, FillRule, RenderContext};

// TODO: this will eventually become a `kurbo::Shape`.
fn circle<V: Into<Vec2>>(center: V, radius: f64, num_segments: usize) -> BezPath {
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
            path.moveto((x + centerx, y + centery));
        } else {
            let end = (x + centerx, y + centery);
            path.lineto(end);
        }
    }

    path.closepath();
    return path;
}

fn draw_cubic_bezier<V: Into<Vec2>>(
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
    path.moveto(p0);
    path.curveto(p1, p2, p3);
    let curve_brush = rc.solid_brush(0x00_80_00_FF)?;
    rc.stroke(&path, &curve_brush, 3.0, None);

    let handle_brush = rc.solid_brush(0x00_00_80_FF)?;
    rc.stroke(&Line::new(p0, p1), &handle_brush, 1.0, None);
    rc.stroke(&Line::new(p2, p3), &handle_brush, 1.0, None);

    for p in [p0, p1, p2, p3].into_iter() {
        let dot = circle(*p, 1.5, 20);
        rc.fill(&dot, &handle_brush, FillRule::NonZero);
    }
    Ok(())
}

pub fn draw<R: RenderContext>(rc: &mut R) -> Result<(), Error> {
    rc.clear(0xFF_FF_FF);
    draw_cubic_bezier(rc, (70.0, 80.0), (140.0, 10.0), (60.0, 10.0), (90.0, 80.0))
}
