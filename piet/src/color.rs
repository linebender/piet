//! A simple representation of color

/// A datatype representing color.
///
/// Currently this is only a 32 bit RGBA value, but it will likely
/// extend to some form of wide-gamut colorspace, and in the meantime
/// is useful for giving programs proper type.
#[derive(Clone)]
pub enum Color {
    Rgba32(u32),
}

impl Color {
    /// Create a color from a 32-bit rgba value (alpha as least significant byte).
    pub const fn rgba32(rgba: u32) -> Color {
        Color::Rgba32(rgba)
    }

    /// Create a color from a 24-bit rgb value (red most significant, blue least).
    pub const fn rgb24(rgb: u32) -> Color {
        Color::rgba32((rgb << 8) | 0xff)
    }

    /// Create a color from four floating point values, each in the range 0.0 to 1.0.
    ///
    /// The interpretation is the same as rgba32, and no greater precision is
    /// (currently) assumed.
    pub fn rgba<F: Into<f64>>(r: F, g: F, b: F, a: F) -> Color {
        let r = (r.into().max(0.0).min(1.0) * 255.0).round() as u32;
        let g = (g.into().max(0.0).min(1.0) * 255.0).round() as u32;
        let b = (b.into().max(0.0).min(1.0) * 255.0).round() as u32;
        let a = (a.into().max(0.0).min(1.0) * 255.0).round() as u32;
        Color::rgba32((r << 24) | (g << 16) | (b << 8) | a)
    }

    /// Create a color from three floating point values, each in the range 0.0 to 1.0.
    ///
    /// The interpretation is the same as rgb24, and no greater precision is
    /// (currently) assumed.
    pub fn rgb<F: Into<f64>>(r: F, g: F, b: F) -> Color {
        let r = (r.into().max(0.0).min(1.0) * 255.0).round() as u32;
        let g = (g.into().max(0.0).min(1.0) * 255.0).round() as u32;
        let b = (b.into().max(0.0).min(1.0) * 255.0).round() as u32;
        Color::rgba32((r << 24) | (g << 16) | (b << 8) | 0xff)
    }

    /// Convert a color value to a 32-bit rgba value.
    pub fn as_rgba32(&self) -> u32 {
        match *self {
            Color::Rgba32(rgba) => rgba,
        }
    }

    /// Opaque white.
    pub const WHITE: Color = Color::rgba32(0xff_ff_ff_ff);

    /// Opaque black.
    pub const BLACK: Color = Color::rgba32(0x00_00_00_ff);
}
