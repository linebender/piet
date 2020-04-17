use crate::dwrite;
use piet::LineMetric;

pub(crate) fn fetch_line_metrics(layout: &dwrite::TextLayout) -> Vec<LineMetric> {
    let mut raw_line_metrics = Vec::new();
    layout.get_line_metrics(&mut raw_line_metrics);

    let metrics: Vec<_> = raw_line_metrics
        .iter()
        .scan((0, 0.0), |(start_offset_agg, height_agg), &line_metric| {
            let start_offset = *start_offset_agg;
            let end_offset = start_offset + line_metric.length as usize;
            let trailing_whitespace = line_metric.trailingWhitespaceLength as usize;

            let cumulative_height = *height_agg + line_metric.height as f64;

            let res = LineMetric {
                start_offset,
                end_offset,
                trailing_whitespace,
                height: line_metric.height as f64,
                cumulative_height,
                baseline: line_metric.baseline as f64,
            };

            // update cumulative state
            *start_offset_agg = end_offset;
            *height_agg = cumulative_height;

            Some(res)
        })
        .collect();

    metrics
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
        font: &D2DFont,
    ) {
        let layout = text_layout
            .new_text_layout(&font, input, width)
            .build()
            .unwrap();
        let line_metrics = fetch_line_metrics(&layout.layout);

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
    fn test_fetch_line_metrics() {
        // Setup input, width, and expected
        let input = "piet text most best";

        let width_small = 30.0;
        let expected_small = vec![
            LineMetric {
                start_offset: 0,
                end_offset: 5,
                trailing_whitespace: 1,
                cumulative_height: 15.960_937_5,
                baseline: 12.949_218_75,
                height: 15.960_937_5,
            },
            LineMetric {
                start_offset: 5,
                end_offset: 10,
                trailing_whitespace: 1,
                cumulative_height: 31.921_875,
                baseline: 12.949_218_75,
                height: 15.960_937_5,
            },
            LineMetric {
                start_offset: 10,
                end_offset: 15,
                trailing_whitespace: 1,
                cumulative_height: 47.882_812_5,
                baseline: 12.949_218_75,
                height: 15.960_937_5,
            },
            LineMetric {
                start_offset: 15,
                end_offset: 19,
                trailing_whitespace: 0,
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
                cumulative_height: 15.960_937_5,
                baseline: 12.949_218_75,
                height: 15.960_937_5,
            },
            LineMetric {
                start_offset: 10,
                end_offset: 19,
                trailing_whitespace: 0,
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
            cumulative_height: 15.960_937_5,
            baseline: 12.949_218_75,
            height: 15.960_937_5,
        }];

        let empty_input = "";
        let expected_empty = vec![LineMetric {
            start_offset: 0,
            end_offset: 0,
            trailing_whitespace: 0,
            cumulative_height: 15.960_937_5,
            baseline: 12.949_218_75,
            height: 15.960_937_5,
        }];

        // setup dwrite layout
        let dwrite = dwrite::DwriteFactory::new().unwrap();
        let mut text = D2DText::new(&dwrite);
        let font = text.new_font_by_name("sans-serif", 12.0).build().unwrap();

        test_metrics_with_width(width_small, expected_small, input, &mut text, &font);
        test_metrics_with_width(width_medium, expected_medium, input, &mut text, &font);
        test_metrics_with_width(width_large, expected_large, input, &mut text, &font);
        test_metrics_with_width(width_small, expected_empty, empty_input, &mut text, &font);
    }
}
