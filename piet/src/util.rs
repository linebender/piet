//! Code useful for multiple backends

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
    }
}
