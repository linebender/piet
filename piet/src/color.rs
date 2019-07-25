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

    /// Create a color from an HLC (aka CIEHLC) specification.
    ///
    /// The `h` parameter is an angle in degrees, with 0 roughly magenta, 90
    /// roughly yellow, 180 roughly cyan, and 270 roughly blue. The `l`
    /// parameter is perceptual luminance, with 0 black and 100 white.
    /// The `c` parameter is a chrominance concentration, with 0 grayscale
    /// and a nominal maximum of 127 (in the future, higher values might
    /// be useful, for high gamut contexts).
    ///
    /// Currently this is just converted into sRGB, but in the future as we
    /// support high-gamut colorspaces, it can be used to specify more colors
    /// or existing colors with a higher accuracy.
    ///
    /// Currently out-of-gamut values are clipped to the nearest sRGB color,
    /// which is perhaps not ideal (the clipping might change the hue). See
    /// https://github.com/d3/d3-color/issues/33 for discussion.
    #[allow(non_snake_case)]
    pub fn hlc<F: Into<f64>>(h: F, l: F, c: F) -> Color {
        // The reverse transformation from Lab to XYZ, see
        // https://en.wikipedia.org/wiki/CIELAB_color_space
        fn f_inv(t: f64) -> f64 {
            let d = 6. / 29.;
            if t > d {
                t.powi(3)
            } else {
                3. * d * d * (t - 4. / 29.)
            }
        }
        let th = h.into() * (std::f64::consts::PI / 180.);
        let c = c.into();
        let a = c * th.cos();
        let b = c * th.sin();
        let L = l.into();
        let ll = (L + 16.) * (1. / 116.);
        // Produce XYZ values scaled to D65 white reference
        let X = 0.9505 * f_inv(ll + a * (1. / 500.));
        let Y = f_inv(ll);
        let Z = 1.0890 * f_inv(ll - b * (1. / 200.));
        // See https://en.wikipedia.org/wiki/SRGB
        let r_lin = 3.2406 * X - 1.5372 * Y - 0.4986 * Z;
        let g_lin = -0.9689 * X + 1.8758 * Y + 0.0415 * Z;
        let b_lin = 0.0557 * X - 0.2040 * Y + 1.0570 * Z;
        fn gamma(u: f64) -> f64 {
            if u <= 0.0031308 {
                12.92 * u
            } else {
                1.055 * u.powf(1. / 2.4) - 0.055
            }
        }
        Color::rgb(gamma(r_lin), gamma(g_lin), gamma(b_lin))
    }

    /// Create a color from an HLC specification and alpha.
    ///
    /// The `a` value represents alpha in the range 0.0 to 1.0.
    pub fn hlca<F: Into<f64>>(h: F, l: F, c: F, a: impl Into<f64>) -> Color {
        Color::hlc(h, c, l).with_alpha(a)
    }

    /// Change just the alpha value of a color.
    ///
    /// The `a` value represents alpha in the range 0.0 to 1.0.
    pub fn with_alpha(self, a: impl Into<f64>) -> Color {
        let a = (a.into().max(0.0).min(1.0) * 255.0).round() as u32;
        Color::rgba32((self.as_rgba32() & !0xff) | a)
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
