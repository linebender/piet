//! Gradient specifications.

use kurbo::Vec2;

use crate::Color;

/// Specification of a gradient.
#[derive(Clone)]
pub enum Gradient {
    /// A linear gradient.
    Linear(LinearGradient),
    /// A radial gradient.
    Radial(RadialGradient),
}

/// Specification of a linear gradient.
#[derive(Clone)]
pub struct LinearGradient {
    /// The start point (corresponding to pos 0.0).
    pub start: Vec2,
    /// The end point (corresponding to pos 1.0).
    pub end: Vec2,
    /// The stops.
    ///
    /// There must be at least two for the gradient to be valid.
    pub stops: Vec<GradientStop>,
}

/// Specification of a radial gradient.
#[derive(Clone)]
pub struct RadialGradient {
    /// The center.
    pub center: Vec2,
    /// The offset of the origin relative to the center.
    pub origin_offset: Vec2,
    /// The radius.
    ///
    /// The circle with this radius from the center corresponds to pos 1.0.
    // TODO: investigate elliptical radius
    pub radius: f64,
    /// The stops (see similar field in [`LinearGradient`](#struct.LinearGradient.html)).
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
