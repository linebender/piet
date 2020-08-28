//! Font families, weights, etcetera

use std::sync::Arc;

/// A reference to a font family.
///
/// This may be either a CSS-style "generic family name", such as "serif"
/// or "monospace", or it can be an explicit family name.
///
/// To use a generic family name, use the provided associated constants:
/// `FontFamily::SERIF`, `FontFamily::SANS_SERIF`, `FontFamily::SYSTEM_UI`,
/// and `FontFamily::MONOSPACE`.
///
/// To use a specific font family you should not construct this type directly;
/// instead you should verify that the desired family exists, via the
/// [`Text::font`] API.
///
/// [`Text::font`]: trait.Text.html#tymethod.font
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontFamily(FontFamilyInner);

/// The inner representation of a font family.
///
/// This is not public API for users of piet; it is exposed for backends only.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[doc(hidden)]
#[non_exhaustive]
pub enum FontFamilyInner {
    Serif,
    SansSerif,
    Monospace,
    SystemUi,
    Named(Arc<str>),
}

/// A font weight, represented as a value in the range 1..=1000.
///
/// This is based on the [CSS `font-weight`] property. In general, you should
/// prefer the constants defined on this type, such as `FontWeight::REGULAR` or
/// `FontWeight::BOLD`.
///
/// [CSS `font-weight`]: https://developer.mozilla.org/en-US/docs/Web/CSS/font-weight
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontWeight(u16);

impl FontFamily {
    /// A san-serif font, such as Arial or Helvetica.
    pub const SANS_SERIF: FontFamily = FontFamily(FontFamilyInner::SansSerif);
    /// A serif font, such as Times New Roman or Charter.
    pub const SERIF: FontFamily = FontFamily(FontFamilyInner::Serif);
    /// The platform's preferred UI font; San Francisco on macOS, and Segoe UI
    /// on recent Windows.
    pub const SYSTEM_UI: FontFamily = FontFamily(FontFamilyInner::SystemUi);
    /// A monospace font.
    pub const MONOSPACE: FontFamily = FontFamily(FontFamilyInner::Monospace);

    /// Create a new font family with a given name, without verifying that it exists.
    ///
    /// This should generally not be used; instead you should create a `FontFamily`
    /// by calling the [`Text::font_family`] method, which verifies that the
    /// family name exists.
    ///
    /// [`Text::font_family`]: trait.Text.html#tymethod.font_family
    pub fn new_unchecked(s: impl Into<Arc<str>>) -> Self {
        FontFamily(FontFamilyInner::Named(s.into()))
    }

    pub fn name(&self) -> &str {
        match &self.0 {
            FontFamilyInner::Serif => "serif",
            FontFamilyInner::SansSerif => "sans-serif",
            FontFamilyInner::SystemUi => "system-ui",
            FontFamilyInner::Monospace => "monospace",
            FontFamilyInner::Named(s) => &s,
        }
    }

    /// Returns `true` if this is a generic font family.
    pub fn is_generic(&self) -> bool {
        !matches!(self.0, FontFamilyInner::Named(_))
    }

    /// Backend-only API; access the inner `FontFamilyInner` enum.
    #[doc(hidden)]
    pub fn inner(&self) -> &FontFamilyInner {
        &self.0
    }
}

impl FontWeight {
    pub const THIN: FontWeight = FontWeight(100);
    pub const HAIRLINE: FontWeight = FontWeight::THIN;

    pub const EXTRA_LIGHT: FontWeight = FontWeight(200);

    pub const LIGHT: FontWeight = FontWeight(300);

    pub const REGULAR: FontWeight = FontWeight(400);
    pub const NORMAL: FontWeight = FontWeight::REGULAR;

    pub const MEDIUM: FontWeight = FontWeight(500);

    pub const SEMI_BOLD: FontWeight = FontWeight(600);

    pub const BOLD: FontWeight = FontWeight(700);

    pub const EXTRA_BOLD: FontWeight = FontWeight(800);

    pub const BLACK: FontWeight = FontWeight(900);
    pub const HEAVY: FontWeight = FontWeight::BLACK;

    pub const EXTRA_BLACK: FontWeight = FontWeight(950);

    /// Create a new `FontWeight` with a custom value.
    ///
    /// Values will be clamped to the range 1..=1000.
    pub fn new(raw: u16) -> FontWeight {
        let raw = raw.min(1000).max(1);
        FontWeight(raw)
    }

    /// Return the raw value as a u16.
    pub const fn to_raw(self) -> u16 {
        self.0
    }
}

impl Default for FontFamily {
    fn default() -> Self {
        FontFamily::SYSTEM_UI
    }
}

impl Default for FontWeight {
    fn default() -> Self {
        FontWeight::REGULAR
    }
}
