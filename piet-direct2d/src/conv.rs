//! Conversions of types into Direct2D

use direct2d::math::{ColorF, Matrix3x2F, Point2F, RectF};

use piet::kurbo::{Affine, Point, Rect, Vec2};

use piet::{Color, Error, GradientStop, LineCap, LineJoin, RoundFrom, RoundInto, StrokeStyle};

use crate::error::WrapError;

/// This is wrapped for coherence reasons.
///
/// TODO: consider using Point2F instead, and moving conversions into kurbo.
pub struct Point2(pub Point2F);

impl From<Point2F> for Point2 {
    #[inline]
    fn from(vec: Point2F) -> Point2 {
        Point2(vec.into())
    }
}

impl From<(f32, f32)> for Point2 {
    #[inline]
    fn from(vec: (f32, f32)) -> Point2 {
        Point2(Point2F::new(vec.0, vec.1))
    }
}

// TODO: Maybe there's some blanket implementation that would cover this and
// not cause coherence problems.
impl RoundFrom<(f32, f32)> for Point2 {
    #[inline]
    fn round_from(vec: (f32, f32)) -> Point2 {
        Point2(Point2F::new(vec.0, vec.1))
    }
}

impl RoundFrom<(f64, f64)> for Point2 {
    #[inline]
    fn round_from(vec: (f64, f64)) -> Point2 {
        Point2(Point2F::new(vec.0 as f32, vec.1 as f32))
    }
}

impl RoundFrom<Point> for Point2 {
    #[inline]
    fn round_from(point: Point) -> Point2 {
        Point2(Point2F::new(point.x as f32, point.y as f32))
    }
}

impl RoundFrom<Vec2> for Point2 {
    #[inline]
    fn round_from(vec: Vec2) -> Point2 {
        Point2(Point2F::new(vec.x as f32, vec.y as f32))
    }
}

impl From<Point2> for Vec2 {
    #[inline]
    fn from(vec: Point2) -> Vec2 {
        Vec2::new(vec.0.x as f64, vec.0.y as f64)
    }
}

pub(crate) fn to_point2f<P: RoundInto<Point2>>(p: P) -> Point2F {
    p.round_into().0
}

/// Can't implement RoundFrom here because both types belong to other
/// crates. Consider moving to kurbo (with windows feature).
pub(crate) fn affine_to_matrix3x2f(affine: Affine) -> Matrix3x2F {
    let a = affine.as_coeffs();
    Matrix3x2F::new([
        [a[0] as f32, a[1] as f32],
        [a[2] as f32, a[3] as f32],
        [a[4] as f32, a[5] as f32],
    ])
}

// TODO: consider adding to kurbo.
pub(crate) fn rect_to_rectf(rect: Rect) -> RectF {
    (
        rect.x0 as f32,
        rect.y0 as f32,
        rect.x1 as f32,
        rect.y1 as f32,
    )
        .into()
}

pub(crate) fn color_to_colorf(color: Color) -> ColorF {
    let rgba = color.as_rgba32();
    (rgba >> 8, ((rgba & 255) as f32) * (1.0 / 255.0)).into()
}

pub(crate) fn gradient_stop_to_d2d(stop: &GradientStop) -> direct2d::brush::gradient::GradientStop {
    direct2d::brush::gradient::GradientStop {
        position: stop.pos,
        color: color_to_colorf(stop.color.clone()),
    }
}

fn convert_line_cap(line_cap: LineCap) -> direct2d::enums::CapStyle {
    match line_cap {
        LineCap::Butt => direct2d::enums::CapStyle::Flat,
        LineCap::Round => direct2d::enums::CapStyle::Round,
        LineCap::Square => direct2d::enums::CapStyle::Square,
        // Discussion topic: Triangle. Exposing that as optional
        // functionality is actually a reasonable argument for this being
        // an associated type rather than a concrete type.
    }
}

fn convert_line_join(line_join: LineJoin) -> direct2d::enums::LineJoin {
    match line_join {
        LineJoin::Miter => direct2d::enums::LineJoin::Miter,
        LineJoin::Round => direct2d::enums::LineJoin::Round,
        LineJoin::Bevel => direct2d::enums::LineJoin::Bevel,
        // Discussion topic: MiterOrBevel. Exposing that as optional
        // functionality is actually a reasonable argument for this being
        // an associated type rather than a concrete type.
    }
}

pub(crate) fn convert_stroke_style(
    factory: &direct2d::Factory,
    stroke_style: &StrokeStyle,
    width: f32,
) -> Result<direct2d::stroke_style::StrokeStyle, Error> {
    #[allow(unused)]
    let mut dashes_f32 = Vec::<f32>::new();
    let mut builder = direct2d::stroke_style::StrokeStyle::create(factory);
    if let Some(join) = stroke_style.line_join {
        builder = builder.with_line_join(convert_line_join(join));
    }
    if let Some(cap) = stroke_style.line_cap {
        let cap = convert_line_cap(cap);
        builder = builder.with_start_cap(cap).with_end_cap(cap);
    }
    // D2D seems to use units of multiples of the stroke width.
    let width_recip = if width == 0.0 { 1.0 } else { width.recip() };
    if let Some((ref dashes, offset)) = stroke_style.dash {
        dashes_f32 = dashes.iter().map(|x| *x as f32 * width_recip).collect();
        builder = builder
            .with_dashes(&dashes_f32)
            .with_dash_offset(offset as f32);
    }
    if let Some(limit) = stroke_style.miter_limit {
        builder = builder.with_miter_limit(limit as f32);
    }
    builder.build().wrap()
}
