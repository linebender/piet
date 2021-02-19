//! Gradients.

use crate::kurbo::{Circle, Point, Rect, RoundedRect, Size, Vec2};
use crate::{
    Color, Error, FixedGradient, FixedLinearGradient, FixedRadialGradient, GradientStop, LineCap,
    LineJoin, RenderContext, StrokeStyle,
};

pub const SIZE: Size = Size::new(400., 200.);

pub fn draw<R: RenderContext>(rc: &mut R) -> Result<(), Error> {
    rc.clear(None, Color::BLACK);

    let mut stroke_style = StrokeStyle::new();
    stroke_style.set_line_cap(LineCap::Round);
    stroke_style.set_line_join(LineJoin::Round);
    stroke_style.set_dash(vec![10.0, 7.0, 8.0, 6.0], 0.0);

    // Shape rendering with radial gradient
    let radial_gradient = rc.gradient(FixedGradient::Radial(FixedRadialGradient {
        center: Point::new(30.0, 30.0),
        origin_offset: Vec2::new(10.0, 10.0),
        radius: 40.0,
        stops: create_gradient_stops(),
    }))?;
    rc.stroke(
        Circle::new(Point::new(30.0, 20.0), 15.0),
        &radial_gradient,
        5.0,
    );
    rc.stroke_styled(
        Circle::new(Point::new(30.0, 60.0), 15.0),
        &radial_gradient,
        5.0,
        &stroke_style,
    );

    // Shape rendering with linear gradient
    let linear_gradient = rc.gradient(FixedGradient::Linear(FixedLinearGradient {
        start: Point::new(60.0, 10.0),
        end: Point::new(100.0, 90.0),
        stops: create_gradient_stops(),
    }))?;
    rc.stroke_styled(
        RoundedRect::new(60.0, 0.0, 100.0, 30.0, (7.0, 7.0, 7.0, 15.0)),
        &linear_gradient,
        5.0,
        &stroke_style,
    );
    stroke_style.set_line_cap(LineCap::Square);
    rc.stroke_styled(
        RoundedRect::new(60.0, 40.0, 100.0, 70.0, 3.0),
        &linear_gradient,
        5.0,
        &stroke_style,
    );
    rc.stroke(Rect::new(60.0, 80.0, 100.0, 100.0), &linear_gradient, 4.0);

    rc.fill(
        RoundedRect::new(115.0, 10.0, 165.0, 40.0, (15.0, 2.0, 2.0, 2.0)),
        &linear_gradient,
    );

    rc.stroke(
        RoundedRect::new(115.0, 50.0, 165.0, 80.0, (2.0, 2.0, 15.0, 2.0)),
        &linear_gradient,
        4.0,
    );

    Ok(())
}

fn create_gradient_stops() -> Vec<GradientStop> {
    vec![
        GradientStop {
            pos: 0.0,
            color: Color::rgb(1.0, 0.0, 0.0),
        },
        GradientStop {
            pos: 0.2,
            color: Color::rgb(1.0, 1.0, 0.0),
        },
        GradientStop {
            pos: 0.4,
            color: Color::rgb(0.0, 1.0, 0.0),
        },
        GradientStop {
            pos: 0.6,
            color: Color::rgb(0.0, 1.0, 1.0),
        },
        GradientStop {
            pos: 0.8,
            color: Color::rgb(0.0, 0.0, 1.0),
        },
        GradientStop {
            pos: 1.0,
            color: Color::rgb(1.0, 0.0, 1.0),
        },
    ]
}
