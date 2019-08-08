//! Gradient specifications.

use std::borrow::Cow;

use kurbo::{Point, Rect, Vec2};

use crate::{IntoBrush, RenderContext};

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
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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
#[derive(Clone, Debug)]
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
#[derive(Debug, Clone)]
pub struct LinearGradient {
    start: UnitPoint,
    end: UnitPoint,
    stops: Vec<GradientStop>,
}

/// A description of a radial gradient in the unit rect, which can be resolved
/// to a fixed gradient.
///
/// The center is given in `UnitPoint` coordinates.
///
/// The `center` and `radius` describe a sphere. `origin` describes the angle
/// of the gradient on that sphere, useful for simulating lighting effects.
/// see [configuring a radial gradient][config] for a fuller explanation.
///
/// By default, `origin` is equal to `center`. This can be changed during construction
/// with the [`with_origin`] builder method.
///
/// The [`ScaleMode`] describes how the gradient is mapped to a non-square
/// rectangle; by default this will expand on the longest axis, but this can
/// be changed with the [`with_scale_mode`] builder method.
///
/// [config]: https://docs.microsoft.com/en-us/windows/win32/direct2d/direct2d-brushes-overview#configuring-a-radial-gradient
/// [`ScaleMode`]: enum.ScaleMode.html
/// [`with_origin`]: struct.RadialGradient.html#method.with_origin
/// [`with_scale_mode`]: struct.RadialGradient.html#method.with_scale_mode
#[derive(Debug, Clone)]
pub struct RadialGradient {
    center: UnitPoint,
    origin: UnitPoint,
    radius: f64,
    stops: Vec<GradientStop>,
    scale_mode: ScaleMode,
}

/// Mappings from the unit square into a non-square rectangle.
#[derive(Debug, Clone)]
pub enum ScaleMode {
    /// The unit 1.0 is mapped to the smaller of width & height. All
    /// values will be mapped without clipping, but the mapped item will
    /// not cover the entire reect.
    Fit,
    /// The unit 1.0 is mapped to the larger of width & height; some
    /// values on the other axis will be clipped.
    Fill,
}

/// A representation of a point relative to a unit rectangle.
#[derive(Debug, Clone, Copy)]
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

impl RadialGradient {
    /// Creates a simple `RadialGradient`. This gradient has the same `origin`
    /// and `center`, and uses the `Fill` [`ScaleMode`]. These attributes can be
    /// modified with the [`with_origin`] and [`with_scale_mode`] builder methods.
    ///
    /// [`ScaleMode`]: enum.ScaleMode.html
    /// [`with_origin`]: struct.RadialGradient.html#method.with_origin
    /// [`with_scale_mode`]: struct.RadialGradient.html#method.with_scale_mode
    pub fn new(center: UnitPoint, radius: f64, stops: impl GradientStops) -> Self {
        RadialGradient {
            center,
            origin: center,
            radius,
            stops: stops.to_vec(),
            scale_mode: ScaleMode::Fill,
        }
    }

    /// A builder-style method for changing the origin of the gradient.
    ///
    /// See the main [`RadialGradient`] docs for an explanation of center vs. origin.
    ///
    /// [`RadialGradient`]: struct.RadialGradient.html
    pub fn with_origin(mut self, origin: UnitPoint) -> Self {
        self.origin = origin;
        self
    }

    /// A builder-style method for changing the [`ScaleMode`] of the gradient.
    ///
    /// [`ScaleMode`]: enum.ScaleMode.html
    pub fn with_scale_mode(mut self, scale_mode: ScaleMode) -> Self {
        self.scale_mode = scale_mode;
        self
    }
}

impl<P: RenderContext> IntoBrush<P> for FixedGradient {
    fn make_brush<'a>(&'a self, piet: &mut P, _bbox: impl FnOnce() -> Rect) -> Cow<'a, P::Brush> {
        // Also, at some point we might want to be smarter about the extra clone here.
        Cow::Owned(
            piet.gradient(self.to_owned())
                .expect("error creating gradient"),
        )
    }
}

impl<P: RenderContext> IntoBrush<P> for LinearGradient {
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

impl<P: RenderContext> IntoBrush<P> for RadialGradient {
    fn make_brush<'a>(&'a self, piet: &mut P, bbox: impl FnOnce() -> Rect) -> Cow<'a, P::Brush> {
        let rect = bbox();
        let scale_len = match self.scale_mode {
            ScaleMode::Fill => rect.width().max(rect.height()),
            ScaleMode::Fit => rect.width().min(rect.height()),
        };

        let rect =  equalize_sides_preserving_center(rect, scale_len);
        let center = self.center.resolve(rect);
        let origin = self.origin.resolve(rect);
        let origin_offset = origin - center;
        let radius = self.radius * scale_len;

        let gradient = FixedGradient::Radial(FixedRadialGradient {
            center,
            origin_offset,
            radius,
            stops: self.stops.clone(),
        });
        // Perhaps the make_brush method should be fallible instead of panicking.
        Cow::Owned(piet.gradient(gradient).expect("error creating gradient"))
    }
}

fn equalize_sides_preserving_center(rect: Rect, new_len: f64) -> Rect {
    let (x, width) = if new_len != rect.width() {
        let dwidth = rect.width() - new_len;
        let dx = dwidth * 0.5;
        (rect.x0 + dx, new_len)
    } else {
        (rect.x0, rect.width())
    };

    let (y, height) = if new_len != rect.height() {
        let dheight = rect.height() - new_len;
        let dy = dheight * 0.5;
        (rect.y0 + dy, new_len)
    } else {
        (rect.y0, rect.height())
    };

    Rect::from_origin_size((x, y), (width, height))
}
