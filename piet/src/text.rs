//! Traits for fonts and text handling.

use crate::kurbo::Point;
use crate::Error;

pub trait Text: Clone {
    type FontBuilder: FontBuilder<Out = Self::Font>;
    type Font: Font;

    type TextLayoutBuilder: TextLayoutBuilder<Out = Self::TextLayout>;
    type TextLayout: TextLayout;

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

pub trait FontBuilder {
    type Out: Font;

    fn build(self) -> Result<Self::Out, Error>;
}

pub trait Font {}

pub trait TextLayoutBuilder {
    type Out: TextLayout;

    /// Set the [`TextAlignment`] to be used for this layout.
    ///
    /// [`TextAlignment`]: enum.TextAlignment.html
    fn alignment(self, alignment: TextAlignment) -> Self;
    fn build(self) -> Result<Self::Out, Error>;
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
