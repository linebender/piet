//! Code useful for multiple backends

use std::ops::{Bound, Range, RangeBounds};

use crate::kurbo::{Rect, Size};
use crate::{Color, FontFamily, FontStyle, FontWeight, LineMetric, TextAttribute};

/// The default point sie for text in piet.
pub const DEFAULT_FONT_SIZE: f64 = 12.0;

/// The default foreground text color.
pub const DEFAULT_TEXT_COLOR: Color = Color::BLACK;

/// Counts the number of utf-16 code units in the given string.
/// from xi-editor
pub fn count_utf16(s: &str) -> usize {
    let mut utf16_count = 0;
    for &b in s.as_bytes() {
        if (b as i8) >= -0x40 {
            utf16_count += 1;
        }
        if b >= 0xf0 {
            utf16_count += 1;
        }
    }
    utf16_count
}

/// returns utf8 text position (code unit offset) at the given utf-16 text position
#[allow(clippy::explicit_counter_loop)]
pub fn count_until_utf16(s: &str, utf16_text_position: usize) -> Option<usize> {
    let mut utf8_count = 0;
    let mut utf16_count = 0;
    for &b in s.as_bytes() {
        if (b as i8) >= -0x40 {
            utf16_count += 1;
        }
        if b >= 0xf0 {
            utf16_count += 1;
        }

        if utf16_count > utf16_text_position {
            return Some(utf8_count);
        }

        utf8_count += 1;
    }

    None
}

/// Returns the index of the line containing this utf8 position,
/// or the last line index if the position is out of bounds.
///
/// `lines` must not be empty.
pub fn line_number_for_position(lines: &[LineMetric], position: usize) -> usize {
    assert!(!lines.is_empty());
    match lines.binary_search_by_key(&position, |item| item.start_offset) {
        Ok(idx) => idx,
        Err(idx) => idx - 1,
    }
}

/// Resolves a `RangeBounds` into a range in the range 0..len.
pub fn resolve_range(range: impl RangeBounds<usize>, len: usize) -> Range<usize> {
    let start = match range.start_bound() {
        Bound::Unbounded => 0,
        Bound::Included(n) => *n,
        Bound::Excluded(n) => *n + 1,
    };

    let end = match range.end_bound() {
        Bound::Unbounded => len,
        Bound::Included(n) => *n + 1,
        Bound::Excluded(n) => *n,
    };

    start.min(len)..end.min(len)
}

/// Extent to which to expand the blur.
const BLUR_EXTENT: f64 = 2.5;

pub fn size_for_blurred_rect(rect: Rect, radius: f64) -> Size {
    let padding = BLUR_EXTENT * radius;
    let rect_padded = rect.inflate(padding, padding);
    let rect_exp = rect_padded.expand();
    rect_exp.size()
}

/// Generate image for a blurred rect, writing it into the provided buffer.
pub fn compute_blurred_rect(rect: Rect, radius: f64, stride: usize, buf: &mut [u8]) -> Rect {
    let radius_recip = radius.recip();
    let xmax = rect.width() * radius_recip;
    let ymax = rect.height() * radius_recip;
    let padding = BLUR_EXTENT * radius;
    let rect_padded = rect.inflate(padding, padding);
    let rect_exp = rect_padded.expand();
    let xfrac = rect_padded.x0 - rect_exp.x0;
    let yfrac = rect_padded.y0 - rect_exp.y0;
    let width = rect_exp.width() as usize;
    let height = rect_exp.height() as usize;
    let strip = (0..width)
        .map(|i| {
            let x = ((i as f64) - (xfrac + padding)) * radius_recip;
            (255.0 * 0.25) * (compute_erf7(x) + compute_erf7(xmax - x))
        })
        .collect::<Vec<_>>();
    {
        for j in 0..height {
            let y = ((j as f64) - (yfrac + padding)) * radius_recip;
            let z = compute_erf7(y) + compute_erf7(ymax - y);
            for i in 0..width {
                buf[j * stride + i] = (z * strip[i]).round() as u8;
            }
        }
    }
    rect_exp
}

// See https://raphlinus.github.io/audio/2018/09/05/sigmoid.html for a little
// explanation of this approximation to the erf function.
fn compute_erf7(x: f64) -> f64 {
    let x = x * std::f64::consts::FRAC_2_SQRT_PI;
    let xx = x * x;
    let x = x + (0.24295 + (0.03395 + 0.0104 * xx) * xx) * (x * xx);
    x / (1.0 + x * x).sqrt()
}

/// A type backends can use to represent the default values for a `TextLayout`
#[non_exhaustive]
pub struct LayoutDefaults {
    pub font: FontFamily,
    pub font_size: f64,
    pub weight: FontWeight,
    pub fg_color: Color,
    pub style: FontStyle,
    pub underline: bool,
}

impl LayoutDefaults {
    /// Set the default value for a given `TextAttribute`.
    pub fn set(&mut self, val: impl Into<TextAttribute>) {
        match val.into() {
            TextAttribute::FontFamily(t) => self.font = t,
            TextAttribute::FontSize(size) => self.font_size = size,
            TextAttribute::Weight(weight) => self.weight = weight,
            TextAttribute::Style(style) => self.style = style,
            TextAttribute::Underline(flag) => self.underline = flag,
            TextAttribute::ForegroundColor(color) => self.fg_color = color,
        }
    }
}

impl Default for LayoutDefaults {
    fn default() -> Self {
        LayoutDefaults {
            font: FontFamily::default(),
            font_size: DEFAULT_FONT_SIZE,
            weight: FontWeight::default(),
            fg_color: DEFAULT_TEXT_COLOR,
            style: FontStyle::default(),
            underline: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_until_utf16() {
        // Notes on this input:
        // 5 code points
        // 5 utf-16 code units
        // 10 utf-8 code units (2/1/3/3/1)
        // 3 graphemes
        let input = "é\u{0023}\u{FE0F}\u{20E3}1"; // #️⃣

        assert_eq!(count_until_utf16(input, 0), Some(0));
        assert_eq!(count_until_utf16(input, 1), Some(2));
        assert_eq!(count_until_utf16(input, 2), Some(3));
        assert_eq!(count_until_utf16(input, 3), Some(6));
        assert_eq!(count_until_utf16(input, 4), Some(9));
        assert_eq!(count_until_utf16(input, 5), None);

        assert_eq!(count_until_utf16("", 0), None);
    }
}
