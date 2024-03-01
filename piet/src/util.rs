// Copyright 2020 the Piet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Code useful for multiple backends

use std::ops::{Bound, Range, RangeBounds};

use crate::kurbo::{Rect, Size};
use crate::{Color, Error, FontFamily, FontStyle, FontWeight, LineMetric, TextAttribute};

use unic_bidi::bidi_class::{BidiClass, BidiClassCategory};

/// The default point size for text in piet.
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

/// If this string ends in a [newline function (NLF)][] (5.8), return the length
/// of the NLF in bytes.
///
/// Note: this is currently just handling the newline character, `\\n`. When a
/// line ends in a newline and a user clicks at the end of that line,
/// you generally want to insert the cursor *before* that character.
///
/// In the future, we may want to respect other separators, as per the
/// unicode spec.
///
/// [newline function (NLF)]: https://www.unicode.org/versions/Unicode13.0.0/ch05.pdf
pub fn trailing_nlf(s: &str) -> Option<usize> {
    if s.as_bytes().last() == Some(&b'\n') {
        Some(1)
    } else {
        None
    }
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

/// Calculate the size required paint a blurred rect.
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
#[allow(missing_docs)]
pub struct LayoutDefaults {
    pub font: FontFamily,
    pub font_size: f64,
    pub weight: FontWeight,
    pub fg_color: Color,
    pub style: FontStyle,
    pub underline: bool,
    pub strikethrough: bool,
}

impl LayoutDefaults {
    /// Set the default value for a given `TextAttribute`.
    pub fn set(&mut self, val: impl Into<TextAttribute>) {
        match val.into() {
            TextAttribute::FontFamily(t) => self.font = t,
            TextAttribute::FontSize(size) => {
                self.font_size = if size <= 0.0 { DEFAULT_FONT_SIZE } else { size }
            }
            TextAttribute::Weight(weight) => self.weight = weight,
            TextAttribute::Style(style) => self.style = style,
            TextAttribute::Underline(flag) => self.underline = flag,
            TextAttribute::TextColor(color) => self.fg_color = color,
            TextAttribute::Strikethrough(flag) => self.strikethrough = flag,
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
            strikethrough: false,
        }
    }
}

/// If `x` is a single (non-alpha) channel of a premultiplied color and `a` is the alpha channel,
/// returns the corresponding channel of the unpremultiplied version of the color.
pub fn unpremul(x: u8, a: u8) -> u8 {
    if a == 0 {
        0
    } else {
        let y = (x as u32 * 255 + (a as u32 / 2)) / (a as u32);
        y.min(255) as u8
    }
}

/// Takes a buffer of premultiplied RGBA pixels and unpremultiplies them in place.
pub fn unpremultiply_rgba(data: &mut [u8]) {
    for i in (0..data.len()).step_by(4) {
        let a = data[i + 3];
        if a != 0 {
            for x in &mut data[i..(i + 3)] {
                *x = unpremul(*x, a);
            }
        }
    }
}

/// A heurstic for text direction; returns `true` if, while enumerating characters
/// in this string, a character in the 'R' (strong right-to-left) category is
/// encountered before any character in the 'L' (strong left-to-right) category is.
///
/// See [Unicode technical report 9](https://unicode.org/reports/tr9/#Table_Bidirectional_Character_Types).
pub fn first_strong_rtl(text: &str) -> bool {
    text.chars()
        // an upper bound on how many chars we'll check
        .take(200)
        .map(BidiClass::of)
        .find(|c| c.category() == BidiClassCategory::Strong)
        .map(|c| c.is_rtl())
        .unwrap_or(false)
}

/// Returns the number of bytes needed to be read from the image buffer.
pub fn expected_image_buffer_size(row_size: usize, height: usize, stride: usize) -> usize {
    if height == 0 {
        return 0;
    }

    stride * (height - 1) + row_size
}

/// Converts an image buffer to a tightly packed owned buffer.
///
/// # Notes
///
/// * strides less than `width * bytes_per_pixel` are **allowed**
/// * if the buffer is too small, [`InvalidInput`] is returned
///
/// [`InvalidInput`]: Error::InvalidInput
pub fn image_buffer_to_tightly_packed(
    buff: &[u8],
    width: usize,
    height: usize,
    stride: usize,
    format: crate::ImageFormat,
) -> Result<Vec<u8>, Error> {
    if width == 0 || height == 0 {
        // empty image
        return Ok(vec![]);
    }

    let bytes_per_pixel = format.bytes_per_pixel();
    let row_size = width * bytes_per_pixel;

    if buff.len() < expected_image_buffer_size(row_size, height, stride) {
        return Err(Error::InvalidInput);
    }

    if stride == row_size {
        // nothing to do, just return the buffer as is
        return Ok(buff.to_vec());
    }

    let mut new_buff = vec![0u8; height * row_size];

    for y in 0..height {
        let src_row_start = y * stride;
        let dst_row_start = y * row_size;

        let src_row_slice = &buff[src_row_start..(src_row_start + row_size)];
        let dst_row_slice = &mut new_buff[dst_row_start..(dst_row_start + row_size)];

        dst_row_slice.copy_from_slice(src_row_slice);
    }

    Ok(new_buff)
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

    #[test]
    fn test_image_buffer_to_tightly_packed() {
        let w: u16 = 7;
        let h: u16 = 11;
        let format = crate::ImageFormat::RgbaSeparate;
        let bpp = format.bytes_per_pixel();

        let make_buffer_with_stride = |stride: usize| -> Vec<u8> {
            let mut buff = vec![0u8; stride * h as usize];

            let split_u16 = |x: u16| -> (u8, u8) {
                let most_significant = (x >> 8) as u8;
                let least_significant = (x & 0xff) as u8;
                (most_significant, least_significant)
            };

            for y in 0..h {
                for x in 0..w {
                    let i: usize = y as usize * stride + x as usize * bpp;
                    let (r, g) = split_u16(x);
                    let (b, a) = split_u16(y);
                    buff[i] = r;
                    buff[i + 1] = g;
                    buff[i + 2] = b;
                    buff[i + 3] = a;
                }
            }

            buff
        };

        // a buffer with the correct stride should be returned as is
        let tightly_packed_buffer = make_buffer_with_stride(w as usize * bpp);
        let result = image_buffer_to_tightly_packed(
            &tightly_packed_buffer,
            w as usize,
            h as usize,
            w as usize * bpp,
            format,
        );
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result, tightly_packed_buffer);

        // a buffer with a larger stride should be converted to tightly packed
        let buff = make_buffer_with_stride(w as usize * bpp + 1);
        let result = image_buffer_to_tightly_packed(
            &buff,
            w as usize,
            h as usize,
            w as usize * bpp + 1,
            format,
        );
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result, tightly_packed_buffer);

        // a buffer that is too small should return an error
        let small_buff = tightly_packed_buffer[..(tightly_packed_buffer.len() - 1)].to_vec();
        let result = image_buffer_to_tightly_packed(
            &small_buff,
            w as usize,
            h as usize,
            w as usize * bpp,
            format,
        );
        assert!(result.is_err());
        let result = result.unwrap_err();
        assert_eq!(result.to_string(), Error::InvalidInput.to_string());
    }
}
