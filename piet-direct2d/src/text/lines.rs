use directwrite::text_layout::TextLayout;
use piet::LineMetric;

pub(crate) fn fetch_line_metrics(layout: &TextLayout) -> Vec<LineMetric> {
    let mut dw_line_metrics = Vec::new();
    layout.get_line_metrics(&mut dw_line_metrics);

    dw_line_metrics.iter()
        .scan(0, |line_start_offset_agg, &line_metric| {
            let line_start_offset = *line_start_offset_agg;
            let line_length_offset = line_start_offset + line_metric.length() as usize;
            let line_length_trailing_whitespace_offset = line_length_offset - line_metric.trailing_whitespace_length() as usize;

            let res = LineMetric {
                line_start_offset,
                line_length_offset,
                line_length_trailing_whitespace_offset,

            };

            *line_start_offset_agg += line_length_offset;

            Some(res)
        })
        .collect()
}
