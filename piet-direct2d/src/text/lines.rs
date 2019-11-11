use directwrite::text_layout::TextLayout as DwTextLayout;
use piet::LineMetric;

pub(crate) fn fetch_line_metrics(layout: &DwTextLayout) -> Vec<LineMetric> {
    let mut dw_line_metrics = Vec::new();
    layout.get_line_metrics(&mut dw_line_metrics);

    dw_line_metrics.iter()
        .scan(0, |line_start_offset_agg, &line_metric| {
            let line_start_offset = *line_start_offset_agg;
            let line_length_trailing_whitespace_offset = line_start_offset + line_metric.length() as usize;
            let line_length_offset = line_length_trailing_whitespace_offset - line_metric.trailing_whitespace_length() as usize;

            let res = LineMetric {
                line_start_offset,
                line_length_offset,
                line_length_trailing_whitespace_offset,

            };

            *line_start_offset_agg = line_length_trailing_whitespace_offset;

            Some(res)
        })
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;
    use super::super::*;

    #[test]
    fn test_fetch_line_metrics() {
        let dwrite = directwrite::factory::Factory::new().unwrap();

        let input = "piet text is the greatest";

        let mut text_layout = D2DText::new(&dwrite);
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout.new_text_layout(&font, input, 25.0).build().unwrap();

        let line_metrics = fetch_line_metrics(&layout.layout);

        println!("{:#?}", layout.line_metrics);
        assert_eq!(line_metrics.len(), 0);
    }
}
