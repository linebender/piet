//! Traits for fonts and text handling.

use crate::kurbo::Point;
use crate::Error;

pub trait Text {
    type FontBuilder: FontBuilder<Out = Self::Font>;
    type Font: Font;

    type TextLayoutBuilder: TextLayoutBuilder<Out = Self::TextLayout>;
    type TextLayout: TextLayout;

    fn new_font_by_name(&mut self, name: &str, size: f64) -> Self::FontBuilder;

    fn new_text_layout(&mut self, font: &Self::Font, text: &str) -> Self::TextLayoutBuilder;
}

pub trait FontBuilder {
    type Out: Font;

    fn build(self) -> Result<Self::Out, Error>;
}

pub trait Font {}

pub trait TextLayoutBuilder {
    type Out: TextLayout;

    fn build(self) -> Result<Self::Out, Error>;
}

/// # Text Layout
///
/// A text position is defined in utf-8 code units, as is standard for Rust strings.
///
/// However, text position is also related to valid cursor positions. Therefore:
/// - The end of a line is a valid text position. e.g. `text.len()` is a valid text position.
/// - If the text position is not at a code point or grapheme boundary, undesirable behavior may
/// occur.
///
pub trait TextLayout {
    /// Measure the advance width of the text.
    fn width(&self) -> f64;

    /// Given a Point, determine the corresponding text position
    fn hit_test_point(&self, point: Point) -> HitTestPoint;

    /// Given a text position, determine the corresponding pixel location
    /// (currently consider the text layout just one line)
    ///
    /// Note: if text position is not at grapheme boundary, rounds the boundary to the text position of the
    /// grapheme cluster it is a part of. Returned text position is the original text position.
    ///
    /// Note: in directwrite, if text position is not at code point boundary, this method will
    /// panic. Cairo and web are more lenient and may not panic.
    fn hit_test_text_position(&self, text_position: usize) -> Option<HitTestTextPosition>;
}

/// return values for [`hit_test_point`](../piet/trait.TextLayout.html#tymethod.hit_test_point).
/// - `metrics.text_position` will give you the text position.
/// - `is_inside` indicates whether the hit test point landed within the text.
#[derive(Debug, Default, PartialEq)]
pub struct HitTestPoint {
    pub metrics: HitTestMetrics,
    pub is_inside: bool,
    // removing until needed for BIDI or other.
    //pub is_trailing_hit: bool,
}

/// return values for [`hit_test_text_position`](../piet/trait.TextLayout.html#tymethod.hit_test_text_position).
#[derive(Debug, Default)]
pub struct HitTestTextPosition {
    pub point: Point,
    pub metrics: HitTestMetrics,
}

#[derive(Debug, Default, PartialEq)]
/// Hit test metrics, returned as part of [`hit_test_text_position`](../piet/trait.TextLayout.html#tymethod.hit_test_text_position)
/// and [`hit_test_point`](../piet/trait.TextLayout.html#tymethod.hit_test_point).
pub struct HitTestMetrics {
    pub text_position: usize,
    pub is_text: bool,
    // TODO:
    // consider adding other metrics as needed, such as those provided in
    // [DWRITE_HIT_TEST_METRICS](https://docs.microsoft.com/en-us/windows/win32/api/dwrite/ns-dwrite-dwrite_hit_test_metrics).
}
