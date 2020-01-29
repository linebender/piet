//! Conversions of types into Direct2D

use winapi::um::d2d1::{
    D2D1_CAP_STYLE, D2D1_CAP_STYLE_FLAT, D2D1_CAP_STYLE_ROUND, D2D1_CAP_STYLE_SQUARE, D2D1_COLOR_F,
    D2D1_DASH_STYLE_CUSTOM, D2D1_DASH_STYLE_SOLID, D2D1_GRADIENT_STOP, D2D1_LINE_JOIN,
    D2D1_LINE_JOIN_BEVEL, D2D1_LINE_JOIN_MITER, D2D1_LINE_JOIN_ROUND, D2D1_MATRIX_3X2_F,
    D2D1_POINT_2F, D2D1_RECT_F, D2D1_STROKE_STYLE_PROPERTIES,
};

use piet::kurbo::{Affine, Point, Rect, Vec2};

use piet::{Color, Error, GradientStop, LineCap, LineJoin, RoundFrom, RoundInto, StrokeStyle};

use crate::d2d::D2DFactory;

/// This is wrapped for coherence reasons.
///
/// TODO: consider using Point2F instead, and moving conversions into kurbo.
pub struct Point2(pub D2D1_POINT_2F);

impl From<(f32, f32)> for Point2 {
    #[inline]
    fn from(vec: (f32, f32)) -> Point2 {
        Point2(D2D1_POINT_2F { x: vec.0, y: vec.1 })
    }
}

// TODO: Maybe there's some blanket implementation that would cover this and
// not cause coherence problems.
impl RoundFrom<(f32, f32)> for Point2 {
    #[inline]
    fn round_from(vec: (f32, f32)) -> Point2 {
        Point2(D2D1_POINT_2F { x: vec.0, y: vec.1 })
    }
}

impl RoundFrom<(f64, f64)> for Point2 {
    #[inline]
    fn round_from(vec: (f64, f64)) -> Point2 {
        Point2(D2D1_POINT_2F {
            x: vec.0 as f32,
            y: vec.1 as f32,
        })
    }
}

impl RoundFrom<Point> for Point2 {
    #[inline]
    fn round_from(point: Point) -> Point2 {
        Point2(D2D1_POINT_2F {
            x: point.x as f32,
            y: point.y as f32,
        })
    }
}

impl RoundFrom<Vec2> for Point2 {
    #[inline]
    fn round_from(vec: Vec2) -> Point2 {
        Point2(D2D1_POINT_2F {
            x: vec.x as f32,
            y: vec.y as f32,
        })
    }
}

impl From<Point2> for Vec2 {
    #[inline]
    fn from(vec: Point2) -> Vec2 {
        Vec2::new(vec.0.x as f64, vec.0.y as f64)
    }
}

pub(crate) fn to_point2f<P: RoundInto<Point2>>(p: P) -> D2D1_POINT_2F {
    p.round_into().0
}

/// Can't implement RoundFrom here because both types belong to other
/// crates. Consider moving to kurbo (with windows feature).
pub(crate) fn affine_to_matrix3x2f(affine: Affine) -> D2D1_MATRIX_3X2_F {
    let a = affine.as_coeffs();
    D2D1_MATRIX_3X2_F {
        matrix: [
            [a[0] as f32, a[1] as f32],
            [a[2] as f32, a[3] as f32],
            [a[4] as f32, a[5] as f32],
        ],
    }
}

// TODO: consider adding to kurbo.
pub(crate) fn rect_to_rectf(rect: Rect) -> D2D1_RECT_F {
    D2D1_RECT_F {
        left: rect.x0 as f32,
        top: rect.y0 as f32,
        right: rect.x1 as f32,
        bottom: rect.y1 as f32,
    }
}

pub(crate) fn color_to_colorf(color: Color) -> D2D1_COLOR_F {
    let rgba = color.as_rgba_u32();
    D2D1_COLOR_F {
        r: (((rgba >> 24) & 255) as f32) * (1.0 / 255.0),
        g: (((rgba >> 16) & 255) as f32) * (1.0 / 255.0),
        b: (((rgba >> 8) & 255) as f32) * (1.0 / 255.0),
        a: ((rgba & 255) as f32) * (1.0 / 255.0),
    }
}

pub(crate) fn gradient_stop_to_d2d(stop: &GradientStop) -> D2D1_GRADIENT_STOP {
    D2D1_GRADIENT_STOP {
        position: stop.pos,
        color: color_to_colorf(stop.color.clone()),
    }
}

fn convert_line_cap(line_cap: LineCap) -> D2D1_CAP_STYLE {
    match line_cap {
        LineCap::Butt => D2D1_CAP_STYLE_FLAT,
        LineCap::Round => D2D1_CAP_STYLE_ROUND,
        LineCap::Square => D2D1_CAP_STYLE_SQUARE,
        // Discussion topic: Triangle. Exposing that as optional
        // functionality is actually a reasonable argument for this being
        // an associated type rather than a concrete type.
    }
}

fn convert_line_join(line_join: LineJoin) -> D2D1_LINE_JOIN {
    match line_join {
        LineJoin::Miter => D2D1_LINE_JOIN_MITER,
        LineJoin::Round => D2D1_LINE_JOIN_ROUND,
        LineJoin::Bevel => D2D1_LINE_JOIN_BEVEL,
        // Discussion topic: MiterOrBevel. Exposing that as optional
        // functionality is actually a reasonable argument for this being
        // an associated type rather than a concrete type.
    }
}

pub(crate) fn convert_stroke_style(
    factory: &D2DFactory,
    stroke_style: &StrokeStyle,
    width: f32,
) -> Result<crate::d2d::StrokeStyle, Error> {
    #[allow(unused)]
    let cap = convert_line_cap(stroke_style.line_cap.unwrap_or(LineCap::Butt));
    let join = convert_line_join(stroke_style.line_join.unwrap_or(LineJoin::Miter));
    let (dashes, dash_style, dash_off) = match &stroke_style.dash {
        Some((dashes, off)) => {
            let width_recip = if width == 0.0 { 1.0 } else { width.recip() };
            assert!(dashes.len() <= 0xffff_ffff);
            (
                Some(dashes.iter().map(|x| *x as f32 * width_recip).collect()),
                D2D1_DASH_STYLE_CUSTOM,
                *off as f32,
            )
        }
        None => (None, D2D1_DASH_STYLE_SOLID, 0.0),
    };
    let props = D2D1_STROKE_STYLE_PROPERTIES {
        startCap: cap,
        endCap: cap,
        dashCap: D2D1_CAP_STYLE_FLAT,
        lineJoin: join,
        miterLimit: stroke_style.miter_limit.unwrap_or(10.0) as f32,
        dashStyle: dash_style,
        dashOffset: dash_off,
    };
    let dashes = dashes.as_ref().map(|v: &Vec<_>| v.as_slice());
    Ok(factory.create_stroke_style(&props, dashes)?)
}
