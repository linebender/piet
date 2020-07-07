//! Traits for fonts and text handling.

use std::ops::RangeBounds;

use crate::kurbo::Point;
use crate::Error;

pub trait Text: Clone {
    type FontBuilder: FontBuilder<Out = Self::Font>;
    type Font: Font;

    type TextLayoutBuilder: TextLayoutBuilder<Font = Self::Font, Out = Self::TextLayout>;
    type TextLayout: TextLayout;

    //TODO: consider supporting "generic" family names? this would make us more CSS
    //like, and could also let us remove the `system_font()` method.
    //What I'm imagining here is a small list of generic family names, like
    //["serif", "sans-serif", "monospace", "system-ui"].

    ///// Attempt to locate a font with a given family name.
    /////
    ///// This should return `Some(Font)` if a font with a matching family name
    ///// is found, and `None` if not.
    /////
    ///// This may be fuzzy; for instance on macOS the "Courier New" family might
    ///// be returned when asking for "courier", for instance.
    //fn font_with_family(&mut self, family_name: &str) -> Option<Self::Font>;

    fn new_font_by_name(&mut self, name: &str, size: f64) -> Self::FontBuilder;

    /// Returns a font suitable for use in UI on this platform.
    fn system_font(&mut self, size: f64) -> Self::Font;

    fn new_text_layout(
        &mut self,
        font: &Self::Font,
        text: &str,
        width: impl Into<Option<f64>>,
    ) -> Self::TextLayoutBuilder;
}

/// A representation of a font.
///
/// This type may not be an actual font object, but is some type that can
/// be resolved to a concrete font.
///
/// When loading a system font, this type can be thought of as a font *family*;
/// if you change the weight or the style in a layout span, that may cause a
/// different font in that family to be used for the actual drawing.
pub trait Font: Clone {}

/// A font weight, represented as a value in the range 1..=1000.
///
/// This is based on the [CSS `font-weight`] property. In general, you should
/// prefer the constants defined on this type, such as `FontWeight::REGULAR` or
/// `FontWeight::BOLD`.
///
/// [CSS `font-weeight`]: https://developer.mozilla.org/en-US/docs/Web/CSS/font-weight
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontWeight(u16);

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

impl Default for FontWeight {
    fn default() -> Self {
        FontWeight::REGULAR
    }
}

pub trait FontBuilder {
    type Out: Font;

    fn build(self) -> Result<Self::Out, Error>;
}

pub trait TextLayoutBuilder {
    type Out: TextLayout;
    type Font: Font;

    /// Set the [`TextAlignment`] to be used for this layout.
    ///
    /// [`TextAlignment`]: enum.TextAlignment.html
    fn alignment(self, alignment: TextAlignment) -> Self;

    fn add_attribute(
        self,
        range: impl RangeBounds<usize>,
        attribute: impl Into<TextAttribute<Self::Font>>,
    ) -> Self;

    fn build(self) -> Result<Self::Out, Error>;
}

/// Attributes that can be applied to text.
pub enum TextAttribute<T> {
    Font(T),
    Size(f64),
    Weight(FontWeight),
    ForegroundColor(crate::Color),
    //BackgroundColor(crate::Color),
    Italic,
    Underline,
}

impl<T: Font> From<T> for TextAttribute<T> {
    fn from(t: T) -> TextAttribute<T> {
        TextAttribute::Font(t)
    }
}

impl<T> From<FontWeight> for TextAttribute<T> {
    fn from(src: FontWeight) -> TextAttribute<T> {
        TextAttribute::Weight(src)
    }
}

impl<T> From<f64> for TextAttribute<T> {
    fn from(src: f64) -> TextAttribute<T> {
        TextAttribute::Size(src)
    }
}

/// The alignment of text in a [`TextLayout`].
///
/// [`TextLayout`]: trait.TextLayout.html
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlignment {
    /// Text is aligned to the left edge in left-to-right scripts, and the
    /// right edge in right-to-left scripts.
    Start,
    /// Text is aligned to the right edge in left-to-right scripts, and the
    /// left edge in right-to-left scripts.
    End,
    Center,
    Justified,
}

impl Default for TextAlignment {
    fn default() -> Self {
        TextAlignment::Start
    }
}

/// # Text Layout
///
/// ## Line Breaks
///
/// A text layout may be broken into multiple lines in order to fit within a given width. Line breaking is generally done
/// between words (whitespace-separated).
///
/// When resizing the width of the text layout, calling [`update_width`][] on the text layout will
/// recalculate line breaks and modify in-place.
///
/// A line's text and [`LineMetric`][]s can be accessed line-by-line by 0-indexed line number.
///
/// Fields on ['LineMetric`] include:
/// - line start offset from text layout beginning (in UTF-8 code units)
/// - line end offset from text layout beginning (in UTF-8 code units)
/// - line trailing whitespace (in UTF-8 code units)
/// - line's baseline, distance of the baseline from the top of the line
/// - line height
/// - cumulative line height (includes previous line heights)
///
/// The trailing whitespace distinction is important. Lines are broken at the grapheme boundary after
/// whitespace, but that whitepace is not necessarily rendered since it's just the trailing
/// whitepace at the end of a line. Keeping the trailing whitespace data available allows API
/// consumers to determine their own trailing whitespace strategy.
///
/// ## Text Position
///
/// A text position is the offset in the underlying string, defined in utf-8 code units, as is standard for Rust strings.
///
/// However, text position is also related to valid cursor positions. Therefore:
/// - The beginning of a line has text position `0`.
/// - The end of a line is a valid text position. e.g. `text.len()` is a valid text position.
/// - If the text position is not at a code point or grapheme boundary, undesirable behavior may
/// occur.
///
/// [`update_width`]: trait.TextLayout.html#tymethod.update_width
/// [`LineMetric`]: struct.LineMetric.html
///
pub trait TextLayout: Clone {
    /// Measure the advance width of the text.
    fn width(&self) -> f64;

    /// Change the width of this `TextLayout`.
    ///
    /// This may be an `f64`, or `None` if this layout is not constrained;
    /// `None` is equivalent to `std::f64::INFINITY`.
    fn update_width(&mut self, new_width: impl Into<Option<f64>>) -> Result<(), Error>;

    /// Given a line number, return a reference to that line's underlying string.
    fn line_text(&self, line_number: usize) -> Option<&str>;

    /// Given a line number, return a reference to that line's metrics.
    fn line_metric(&self, line_number: usize) -> Option<LineMetric>;

    /// Returns total number of lines in the text layout.
    fn line_count(&self) -> usize;

    /// Given a `Point`, determine the corresponding text position.
    ///
    /// ## Return value:
    /// Returns a [`HitTestPoint`][] describing the results of the test.
    ///
    /// [`HitTestPoint`][] field `is_inside` is true if the tested point falls within the bounds of the text, `false` otherwise.
    ///
    /// [`HitTestPoint`][] field `metrics` is a [`HitTestMetrics`][] struct. [`HitTestMetrics`][] field `text_position` is the text
    /// position closest to the tested point.
    ///
    /// ## Notes:
    ///
    /// Some text position will always be returned; if the tested point is inside, it returns the appropriate text
    /// position; if it's outside, it will return the nearest text position (either `0` or `text.len()`).
    ///
    /// For more on text positions, see docs for the [`TextLayout`](../piet/trait.TextLayout.html)
    /// trait.
    ///
    /// [`HitTestPoint`]: struct.HitTestPoint.html
    /// [`HitTestMetrics`]: struct.HitTestMetrics.html
    fn hit_test_point(&self, point: Point) -> HitTestPoint;

    /// Given a text position, determine the corresponding pixel location.
    /// (currently consider the text layout just one line)
    ///
    /// ## Return value:
    /// Returns a [`HitTestTextPosition`][] describing the results of the test.
    ///
    /// [`HitTestTextPosition`][] field `point` is the point offset of the boundary of the
    /// grapheme cluster that the text position is a part of.
    ///
    /// [`HitTestTextPosition`][] field `metrics` is a [`HitTestMetrics`][] struct. [`HitTestMetrics`][] field `text_position` is the original text position (unless out of bounds).
    ///
    /// ## Notes:
    /// In directwrite, if a text position is not at code point boundary, this method will panic.
    /// Cairo and web are more lenient and may not panic.
    ///
    /// For text position that is greater than `text.len()`, web/cairo will return the
    /// [`HitTestTextPosition`][] as if `text_position == text.len()`. In directwrite, the method will
    /// panic, as the text position is out of bounds.
    ///
    /// For more on text positions, see docs for the [`TextLayout`](../piet/trait.TextLayout.html)
    /// trait.
    ///
    /// [`HitTestTextPosition`]: struct.HitTestTextPosition.html
    /// [`HitTestMetrics`]: struct.HitTestMetrics.html
    fn hit_test_text_position(&self, text_position: usize) -> Option<HitTestTextPosition>;
}

/// Metadata about each line in a text layout.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LineMetric {
    /// Index (in code units) of the start of the line, offset from the beginning of the text.
    pub start_offset: usize,

    /// Line length (in UTF-8 code units), but offset from the beginning of the text. So it's the length
    /// of this line summed with the lengths of all the lines before it.
    ///
    /// Includes trailing whitespace.
    pub end_offset: usize,

    /// Length in (in UTF-8 code units) of current line's trailing whitespace.
    pub trailing_whitespace: usize,

    /// Distance of the baseline from the top of the line
    pub baseline: f64,

    /// Line height
    pub height: f64,

    /// Cumulative line height (includes previous line heights)
    pub cumulative_height: f64,
}

/// return values for [`hit_test_point`](../piet/trait.TextLayout.html#tymethod.hit_test_point).
#[derive(Debug, Default, PartialEq)]
pub struct HitTestPoint {
    /// `metrics.text_position` will give you the text position.
    pub metrics: HitTestMetrics,
    /// `is_inside` indicates whether the hit test point landed within the text.
    pub is_inside: bool,
    // removing until needed for BIDI or other.
    //pub is_trailing_hit: bool,
}

/// return values for [`hit_test_text_position`](../piet/trait.TextLayout.html#tymethod.hit_test_text_position).
#[derive(Debug, Default)]
pub struct HitTestTextPosition {
    /// the `point`'s `x` value is the position of the leading edge of the grapheme cluster containing the text position.
    pub point: Point,
    /// `metrics.text_position` will give you the text position.
    pub metrics: HitTestMetrics,
}

#[derive(Debug, Default, PartialEq)]
/// Hit test metrics, returned as part of [`hit_test_text_position`](../piet/trait.TextLayout.html#tymethod.hit_test_text_position)
/// and [`hit_test_point`](../piet/trait.TextLayout.html#tymethod.hit_test_point).
pub struct HitTestMetrics {
    pub text_position: usize,
    // TODO:
    // consider adding other metrics as needed, such as those provided in
    // [DWRITE_HIT_TEST_METRICS](https://docs.microsoft.com/en-us/windows/win32/api/dwrite/ns-dwrite-dwrite_hit_test_metrics).
}
