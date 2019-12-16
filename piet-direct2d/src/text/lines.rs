use directwrite::text_layout::TextLayout as DwTextLayout;
use piet::LineMetric;

pub(crate) fn fetch_line_metrics(layout: &DwTextLayout) -> Vec<LineMetric> {
    let mut dw_line_metrics = Vec::new();
    layout.get_line_metrics(&mut dw_line_metrics);

    let mut metrics: Vec<_> = dw_line_metrics.iter()
        .scan((0, 0.0), |(line_start_offset_agg, height_agg), &line_metric| {
            let line_start_offset = *line_start_offset_agg;
            let line_length_trailing_whitespace_offset = line_start_offset + line_metric.length() as usize;
            let line_length_offset = line_length_trailing_whitespace_offset - line_metric.trailing_whitespace_length() as usize;

            let cum_height = *height_agg + line_metric.height() as f64;

            let res = LineMetric {
                line_start_offset,
                line_length_offset,
                line_length_trailing_whitespace_offset,
                cum_height,

            };

            // update cumulative state
            *line_start_offset_agg = line_length_trailing_whitespace_offset;
            *height_agg = cum_height;

            Some(res)
        })
        .collect();

    // dwrite adds a null terminator to string, so the last index should be shortened by one.
    // Assume that there must be at least one layout item?
    if let Some(last) = metrics.last_mut() {
        last.line_length_offset -= 1;
        last.line_length_trailing_whitespace_offset -= 1;
    }

    metrics
}

#[cfg(test)]
mod test {
    use super::*;
    use super::super::*;

    fn test_metrics_with_width(width: f64, expected: Vec<LineMetric>, input: &str, text_layout: &mut D2DText, font: &D2DFont) {
        let layout = text_layout.new_text_layout(&font, input, width).build().unwrap();
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
                line_start_offset: 0,
                line_length_offset: 4,
                line_length_trailing_whitespace_offset: 5,
                cum_height: 15.9609375,
            },
            LineMetric {
                line_start_offset: 5,
                line_length_offset: 9,
                line_length_trailing_whitespace_offset: 10,
                cum_height: 31.921875,
            },
            LineMetric {
                line_start_offset: 10,
                line_length_offset: 14,
                line_length_trailing_whitespace_offset: 15,
                cum_height: 47.8828125,
            },
            LineMetric {
                line_start_offset: 15,
                line_length_offset: 19,
                line_length_trailing_whitespace_offset: 19,
                cum_height: 63.84375,
            },
        ];

        let width_medium = 60.0;
        let expected_medium = vec![
            LineMetric {
                line_start_offset: 0,
                line_length_offset: 9,
                line_length_trailing_whitespace_offset: 10,
                cum_height: 15.9609375,
            },
            LineMetric {
                line_start_offset: 10,
                line_length_offset: 19,
                line_length_trailing_whitespace_offset: 19,
                cum_height: 31.921875,
            },
        ];

        let width_large = 100.0;
        let expected_large = vec![
            LineMetric {
                line_start_offset: 0,
                line_length_offset: 19,
                line_length_trailing_whitespace_offset: 19,
                cum_height: 15.9609375,
            },
        ];

        let empty_input = "";
        let expected_empty = vec![
            LineMetric {
                line_start_offset: 0,
                line_length_offset: 0,
                line_length_trailing_whitespace_offset: 0,
                cum_height: 15.9609375,
            },
        ];

        // setup dwrite layout
        let dwrite = directwrite::factory::Factory::new().unwrap();
        let mut text_layout = D2DText::new(&dwrite);
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();

        test_metrics_with_width(width_small, expected_small, input, &mut text_layout, &font);
        test_metrics_with_width(width_medium, expected_medium, input, &mut text_layout, &font);
        test_metrics_with_width(width_large, expected_large, input, &mut text_layout, &font);
        test_metrics_with_width(width_small, expected_empty, empty_input, &mut text_layout, &font);
    }
}
