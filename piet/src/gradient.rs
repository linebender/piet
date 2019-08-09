//! Gradient specifications.

use std::borrow::Cow;

use kurbo::{Point, Rect, Vec2};

use crate::{IBrush, RenderContext};

use crate::Color;

/// Specification of a gradient.
///
/// This specification is in terms of image-space coordinates. For linear
/// gradients, in many cases, it is better to specify coordinates relative
/// to the `Rect` of the item being drawn; for these, use [`LinearGradient`]
/// instead. A similar feature for radial gradients doesn't currently exist,
/// but may be added later if needed.
///
/// [`LinearGradient`]: struct.LinearGradient.html
#[derive(Clone)]
pub enum FixedGradient {
    /// A linear gradient.
    Linear(FixedLinearGradient),
    /// A radial gradient.
    Radial(FixedRadialGradient),
}

/// Specification of a linear gradient.
///
/// This specification is in terms of image-space coordinates. In many
/// cases, it is better to specify coordinates relative to the `Rect`
/// of the item being drawn; for these, use [`LinearGradient`] instead.
///
/// [`LinearGradient`]: struct.LinearGradient.html
#[derive(Clone)]
pub struct FixedLinearGradient {
    /// The start point (corresponding to pos 0.0).
    pub start: Point,
    /// The end point (corresponding to pos 1.0).
    pub end: Point,
    /// The stops.
    ///
    /// There must be at least two for the gradient to be valid.
    pub stops: Vec<GradientStop>,
}

/// Specification of a radial gradient.
#[derive(Clone)]
pub struct FixedRadialGradient {
    /// The center.
    pub center: Point,
    /// The offset of the origin relative to the center.
    pub origin_offset: Vec2,
    /// The radius.
    ///
    /// The circle with this radius from the center corresponds to pos 1.0.
    // TODO: investigate elliptical radius
    pub radius: f64,
    /// The stops (see similar field in [`LinearGradient`](struct.LinearGradient.html)).
    pub stops: Vec<GradientStop>,
}

/// Specification of a gradient stop.
#[derive(Clone)]
pub struct GradientStop {
    /// The coordinate of the stop.
    pub pos: f32,
    /// The color at that stop.
    pub color: Color,
}

/// A flexible, ergonomic way to describe gradient stops.
pub trait GradientStops {
    fn to_vec(self) -> Vec<GradientStop>;
}

/// A description of a linear gradient in the unit rect, which can be resolved
/// to a fixed gradient.
///
/// The start and end points in the gradient are given in [`UnitPoint`] coordinates,
/// which are then resolved to image-space coordinates for any given concrete `Rect`.
///
/// When the fixed coordinates are known, use [`FixedGradient`] instead.
///
/// [`UnitPoint`]: struct.UnitPoint.html
/// [`FixedGradient`]: struct.FixedGradient.html
pub struct LinearGradient {
    start: UnitPoint,
    end: UnitPoint,
    stops: Vec<GradientStop>,
}

/// A representation of a point relative to a unit rectangle.
pub struct UnitPoint {
    u: f64,
    v: f64,
}

impl GradientStops for Vec<GradientStop> {
    fn to_vec(self) -> Vec<GradientStop> {
        self
    }
}

impl<'a> GradientStops for &'a [GradientStop] {
    fn to_vec(self) -> Vec<GradientStop> {
        self.to_owned()
    }
}

// Generate equally-spaced stops.
impl<'a> GradientStops for &'a [Color] {
    fn to_vec(self) -> Vec<GradientStop> {
        if self.is_empty() {
            Vec::new()
        } else {
            let denom = (self.len() - 1).max(1) as f32;
            self.iter()
                .enumerate()
                .map(|(i, c)| GradientStop {
                    pos: (i as f32) / denom,
                    color: c.to_owned(),
                })
                .collect()
        }
    }
}

impl<'a> GradientStops for (Color, Color) {
    fn to_vec(self) -> Vec<GradientStop> {
        let stops: &[Color] = &[self.0, self.1];
        GradientStops::to_vec(stops)
    }
}

impl<'a> GradientStops for (Color, Color, Color) {
    fn to_vec(self) -> Vec<GradientStop> {
        let stops: &[Color] = &[self.0, self.1, self.2];
        GradientStops::to_vec(stops)
    }
}

impl<'a> GradientStops for (Color, Color, Color, Color) {
    fn to_vec(self) -> Vec<GradientStop> {
        let stops: &[Color] = &[self.0, self.1, self.2, self.3];
        GradientStops::to_vec(stops)
    }
}

impl<'a> GradientStops for (Color, Color, Color, Color, Color) {
    fn to_vec(self) -> Vec<GradientStop> {
        let stops: &[Color] = &[self.0, self.1, self.2, self.3, self.4];
        GradientStops::to_vec(stops)
    }
}

impl<'a> GradientStops for (Color, Color, Color, Color, Color, Color) {
    fn to_vec(self) -> Vec<GradientStop> {
        let stops: &[Color] = &[self.0, self.1, self.2, self.3, self.4, self.5];
        GradientStops::to_vec(stops)
    }
}

impl UnitPoint {
    pub const TOP_LEFT: UnitPoint = UnitPoint::new(0.0, 0.0);
    pub const TOP: UnitPoint = UnitPoint::new(0.5, 0.0);
    pub const TOP_RIGHT: UnitPoint = UnitPoint::new(1.0, 0.0);
    pub const LEFT: UnitPoint = UnitPoint::new(0.0, 0.5);
    pub const CENTER: UnitPoint = UnitPoint::new(0.5, 0.5);
    pub const RIGHT: UnitPoint = UnitPoint::new(1.0, 0.5);
    pub const BOTTOM_LEFT: UnitPoint = UnitPoint::new(0.0, 1.0);
    pub const BOTTOM: UnitPoint = UnitPoint::new(0.5, 1.0);
    pub const BOTTOM_RIGHT: UnitPoint = UnitPoint::new(1.0, 1.0);

    /// Create a new UnitPoint.
    ///
    /// The `u` and `v` coordinates describe the point, with (0.0, 0.0) being
    /// the top-left, and (1.0, 1.0) being the bottom-right.
    pub const fn new(u: f64, v: f64) -> UnitPoint {
        UnitPoint { u, v }
    }

    /// Given a rectangle, resolve the point within the rectangle.
    pub fn resolve(&self, rect: Rect) -> Point {
        Point::new(
            rect.x0 + self.u * (rect.x1 - rect.x0),
            rect.y0 + self.v * (rect.y1 - rect.y0),
        )
    }
}

impl LinearGradient {
    /// Create a new linear gradient.
    ///
    /// The `start` and `end` coordinates are [`UnitPoint`] coordinates, relative
    /// to the geometry of the shape being drawn.
    ///
    /// # Examples
    ///
    /// ```
    /// use piet::{Color, RenderContext, LinearGradient, UnitPoint};
    /// use piet::kurbo::{Circle, Point};
    ///
    /// # let mut render_ctx = piet::NullRenderContext::new();
    /// let circle = Circle::new(Point::new(100.0, 100.0), 50.0);
    /// let gradient = LinearGradient::new(
    ///     UnitPoint::TOP,
    ///     UnitPoint::BOTTOM,
    ///     (Color::WHITE, Color::BLACK)
    /// );
    /// render_ctx.fill(circle, &gradient);
    /// ```
    ///
    /// [`UnitPoint`]: struct.UnitPoint.html
    pub fn new(start: UnitPoint, end: UnitPoint, stops: impl GradientStops) -> LinearGradient {
        LinearGradient {
            start,
            end,
            stops: stops.to_vec(),
        }
    }
}

impl<P: RenderContext> IBrush<P> for LinearGradient {
    fn make_brush<'a>(&'a self, piet: &mut P, bbox: impl FnOnce() -> Rect) -> Cow<'a, P::Brush> {
        let rect = bbox();
        let gradient = FixedGradient::Linear(FixedLinearGradient {
            start: self.start.resolve(rect),
            end: self.end.resolve(rect),
            stops: self.stops.clone(),
        });
        // Perhaps the make_brush method should be fallible instead of panicking.
        Cow::Owned(piet.gradient(gradient).expect("error creating gradient"))
    }
}

impl<P: RenderContext> IBrush<P> for FixedGradient {
    fn make_brush<'a>(&'a self, piet: &mut P, _bbox: impl FnOnce() -> Rect) -> Cow<'a, P::Brush> {
        // Perhaps the make_brush method should be fallible instead of panicking.
        // Also, at some point we might want to be smarter about the extra clone here.
        Cow::Owned(
            piet.gradient(self.to_owned())
                .expect("error creating gradient"),
        )
    }
}
