//! Gradients.

use crate::kurbo::{Circle, Point, Rect, RoundedRect, Vec2};
use crate::{
    Color, Error, FixedGradient, FixedLinearGradient, FixedRadialGradient, FontBuilder,
    GradientStop, LineCap, LineJoin, RenderContext, StrokeStyle, Text, TextLayoutBuilder,
};

pub fn draw<R: RenderContext>(rc: &mut R) -> Result<(), Error> {
    rc.clear(Color::BLACK);

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
        RoundedRect::new(60.0, 0.0, 100.0, 30.0, 7.0),
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

    // Text rendering with gradients
    let linear_gradient = rc.gradient(FixedGradient::Linear(FixedLinearGradient {
        start: Point::new(120.0, 20.0),
        end: Point::new(150.0, 50.0),
        stops: create_gradient_stops(),
    }))?;
    let radial_gradient = rc.gradient(FixedGradient::Radial(FixedRadialGradient {
        center: Point::new(130.0, 70.0),
        origin_offset: Vec2::new(30.0, 10.0),
        radius: 40.0,
        stops: create_gradient_stops(),
    }))?;

    let jp_font = rc.text().new_font_by_name("Noto Sans JP", 8.0).build()?;
    let jp_text = rc
        .text()
        .new_text_layout(&jp_font, "ローカリゼーション作品", None)
        .build()?;
    let font = rc.text().new_font_by_name("Segoe UI", 8.0).build()?;
    let en_text = rc
        .text()
        .new_text_layout(&font, "Text with gradient", None)
        .build()?;

    // Linear gradient
    rc.draw_text(&jp_text, Point::new(110.0, 10.0), &linear_gradient);
    rc.draw_text(&en_text, Point::new(110.0, 20.0), &linear_gradient);
    rc.draw_text(&jp_text, Point::new(110.0, 30.0), &linear_gradient);
    rc.draw_text(&en_text, Point::new(110.0, 40.0), &linear_gradient);

    // Radial gradient
    rc.draw_text(&jp_text, Point::new(110.0, 60.0), &radial_gradient);
    rc.draw_text(&en_text, Point::new(110.0, 70.0), &radial_gradient);
    rc.draw_text(&jp_text, Point::new(110.0, 80.0), &radial_gradient);
    rc.draw_text(&en_text, Point::new(110.0, 90.0), &radial_gradient);
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
