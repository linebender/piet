use crate::dwrite;
use piet::{util, LineMetric};

pub(crate) fn fetch_line_metrics(text: &str, layout: &dwrite::TextLayout) -> Vec<LineMetric> {
    let mut raw_line_metrics = Vec::new();
    layout.get_line_metrics(&mut raw_line_metrics);

    let mut offset_utf8 = 0;
    let mut cumulative_height = 0.0;

    let mut out = Vec::with_capacity(raw_line_metrics.len());

    for raw_metric in raw_line_metrics {
        // this may/will panic if `text` is not the text used to create this layout.
        let (non_ws_len_8, ws_len_8) = len_and_ws_len_utf8(
            &text[offset_utf8..],
            raw_metric.length,
            raw_metric.trailingWhitespaceLength,
        );

        let end_offset = offset_utf8 + non_ws_len_8 + ws_len_8;
        let y_offset = cumulative_height;
        cumulative_height += raw_metric.height as f64;

        #[allow(deprecated)]
        let metric = LineMetric {
            start_offset: offset_utf8,
            end_offset,
            trailing_whitespace: ws_len_8,
            height: raw_metric.height as f64,
            y_offset,
            cumulative_height,
            baseline: raw_metric.baseline as f64,
        };

        offset_utf8 = end_offset;
        out.push(metric);
    }
    out
}

// handles the weirdness where we're dealing with lengths but count_until_utf16 deals
// with offsets
fn len_and_ws_len_utf8(s: &str, total_len_16: u32, ws_len_16: u32) -> (usize, usize) {
    let non_ws_len_16 = (total_len_16 - ws_len_16) as usize;
    let non_ws_len_8 = util::count_until_utf16(s, non_ws_len_16).unwrap_or_else(|| s.len());
    let s = &s[non_ws_len_8..];
    let ws_len_8 = util::count_until_utf16(s, ws_len_16 as usize).unwrap_or_else(|| s.len());
    (non_ws_len_8, ws_len_8)
}

#[cfg(test)]
mod test {
    use super::super::*;
    use super::*;

    fn test_metrics_with_width(
        width: f64,
        expected: Vec<LineMetric>,
        input: &str,
        text_layout: &mut D2DText,
        font: &FontFamily,
        font_size: f64,
    ) {
        let layout = text_layout
            .new_text_layout(input)
            .max_width(width)
            .font(font.clone(), font_size)
            .build()
            .unwrap();
        let line_metrics = fetch_line_metrics(input, &layout.layout);

        println!("{:#?}", layout.line_metrics);
        assert_eq!(line_metrics, expected);
    }

    // Test at three different widths: small, medium, large.
    // - small is every word being split.
    // - medium is one split.
    // - large is no split.
    //
    // Also test empty string input
    //
    // dwrite may split even smaller than a word (hyphenation?), but we
    // don't want to worry about that here yet. TODO
    //
    // TODO figure out how to deal with height floats
    #[test]
    #[allow(deprecated)]
    fn test_fetch_line_metrics() {
        // Setup input, width, and expected
        let input = "piet text most best";

        let width_small = 30.0;
        let expected_small = vec![
            LineMetric {
                start_offset: 0,
                end_offset: 5,
                trailing_whitespace: 1,
                y_offset: 0.0,
                cumulative_height: 15.960_937_5,
                baseline: 12.949_218_75,
                height: 15.960_937_5,
            },
            LineMetric {
                start_offset: 5,
                end_offset: 10,
                trailing_whitespace: 1,
                y_offset: 15.960_937_5,
                cumulative_height: 31.921_875,
                baseline: 12.949_218_75,
                height: 15.960_937_5,
            },
            LineMetric {
                start_offset: 10,
                end_offset: 15,
                trailing_whitespace: 1,
                y_offset: 31.921_875,
                cumulative_height: 47.882_812_5,
                baseline: 12.949_218_75,
                height: 15.960_937_5,
            },
            LineMetric {
                start_offset: 15,
                end_offset: 19,
                trailing_whitespace: 0,
                y_offset: 47.882_812_5,
                cumulative_height: 63.843_75,
                baseline: 12.949_218_75,
                height: 15.960_937_5,
            },
        ];

        let width_medium = 60.0;
        let expected_medium = vec![
            LineMetric {
                start_offset: 0,
                end_offset: 10,
                trailing_whitespace: 1,
                y_offset: 0.0,
                cumulative_height: 15.960_937_5,
                baseline: 12.949_218_75,
                height: 15.960_937_5,
            },
            LineMetric {
                start_offset: 10,
                end_offset: 19,
                trailing_whitespace: 0,
                y_offset: 15.960_937_5,
                cumulative_height: 31.921_875,
                baseline: 12.949_218_75,
                height: 15.960_937_5,
            },
        ];

        let width_large = 100.0;
        let expected_large = vec![LineMetric {
            start_offset: 0,
            end_offset: 19,
            trailing_whitespace: 0,
            y_offset: 0.0,
            cumulative_height: 15.960_937_5,
            baseline: 12.949_218_75,
            height: 15.960_937_5,
        }];

        let empty_input = "";
        let expected_empty = vec![LineMetric {
            start_offset: 0,
            end_offset: 0,
            trailing_whitespace: 0,
            y_offset: 0.0,
            cumulative_height: 15.960_937_5,
            baseline: 12.949_218_75,
            height: 15.960_937_5,
        }];

        // setup dwrite layout
        let mut text = D2DText::new_for_test();
        let font = text.font_family("Segoe UI").unwrap();

        test_metrics_with_width(width_small, expected_small, input, &mut text, &font, 12.0);
        test_metrics_with_width(width_medium, expected_medium, input, &mut text, &font, 12.0);
        test_metrics_with_width(width_large, expected_large, input, &mut text, &font, 12.0);
        test_metrics_with_width(
            width_small,
            expected_empty,
            empty_input,
            &mut text,
            &font,
            12.0,
        );
    }

    #[test]
    fn test_string_range() {
        let input = "€tf\n16";

        let mut text = D2DText::new_for_test();
        let layout = text.new_text_layout(input).build().unwrap();
        let metric = layout.line_metric(0).unwrap();
        assert_eq!(&input[metric.range()], "€tf\n");
        let metric = layout.line_metric(1).unwrap();
        assert_eq!(&input[metric.range()], "16");
    }
}
