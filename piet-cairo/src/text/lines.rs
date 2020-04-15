use cairo::ScaledFont;
use xi_unicode::LineBreakIterator;

use super::LineMetric;

pub(crate) fn calculate_line_metrics(text: &str, font: &ScaledFont, width: f64) -> Vec<LineMetric> {
    // first pass, completely naive and inefficient. Check at every break to see if line longer
    // than width.
    //
    // See https://raphlinus.github.io/rust/skribo/text/2019/04/26/skribo-progress.html for
    // some other ideas for better efficiency
    //
    // Notes:
    // hard breaks: cr, lf, line break, para char. Mandated by unicode
    //
    // So, every time there's a a hard break, must break.
    //
    // soft-hyphen, don't need to deal with this round. Looks like automatic hyphenation, but
    // with unicode codepoint. Don't special case, let if break for now.
    //
    // For soft breaks, then I need to check line widths etc.
    //
    // - what happens when even smallest break is wider than width?
    // One word is considered the smallest unit, don't break below words for now.
    //
    // Use font extents height (it's different from text extents height,
    // which relates to bounding box)
    //
    // For baseline, use use `FontExtent.ascent`. Needs to be positive?
    // see https://glyphsapp.com/tutorials/vertical-metrics
    // https://stackoverflow.com/questions/27631736/meaning-of-top-ascent-baseline-descent-bottom-and-leading-in-androids-font
    // https://www.cairographics.org/manual/cairo-cairo-scaled-font-t.html#cairo-font-extents-t
    let mut line_metrics = Vec::new();
    let mut line_start = 0;
    let mut prev_break = 0;
    let mut cumulative_height = 0.0;

    // vertical measures constant across all lines for now (cairo toy text)
    let height = font.extents().height;
    let baseline = font.extents().ascent;

    for (line_break, is_hard_break) in LineBreakIterator::new(text) {
        if !is_hard_break {
            // this section is for soft breaks
            let curr_str = &text[line_start..line_break];
            let curr_width = font.text_extents(curr_str).x_advance;

            if curr_width > width {
                // since curr_width is longer than desired line width, it's time to break ending
                // at the previous break.

                // Except! what if this break is at first possible break. Then prev_break needs to
                // be moved to current break.
                // This leads to an extra call, in next section, on an empty string for handling
                // prev_break..line_break. But it's a little clearer without more logic, and when
                // perf matters all of this will be rewritten anyways. If desired otherwise, add
                // in a flag after next add_line_metric.
                if prev_break == line_start {
                    prev_break = line_break;
                }

                // first do the line to prev break
                add_line_metric(
                    text,
                    line_start,
                    prev_break,
                    baseline,
                    height,
                    &mut cumulative_height,
                    &mut line_metrics,
                );

                // Now handle the graphemes between prev_break and current break. The
                // implementation depends on how we're treating a single line that's wider than
                // desired width. For now, just assume that the word will get cutoff when rendered.
                //
                // If it's shorter than desired width, just continue.

                let curr_str = &text[prev_break..line_break];
                let curr_width = font.text_extents(curr_str).x_advance;

                if curr_width > width {
                    add_line_metric(
                        text,
                        prev_break,
                        line_break,
                        baseline,
                        height,
                        &mut cumulative_height,
                        &mut line_metrics,
                    );

                    line_start = line_break;
                    prev_break = line_break;
                } else {
                    // Since curr_width < width, don't break and just continue
                    line_start = prev_break;
                    prev_break = line_break;
                }
            } else {
                // Since curr_width < width, don't break and just continue
                prev_break = line_break;
            }
        } else {
            // this section is for hard breaks

            // even when there's a hard break, need to check first to see if width is too wide. If
            // it is, need to break at the previous soft break first.
            let curr_str = &text[line_start..line_break];
            let curr_width = font.text_extents(curr_str).x_advance;

            if curr_width > width {
                // if line is too wide but can't break down anymore, just skip to the next
                // add_line_metric. But here, since prev_break is not equal to line_start, that
                // means there another break opportunity so take it.
                //
                // TODO consider refactoring to make more parallel with above soft break
                // comparison.
                if prev_break != line_start {
                    add_line_metric(
                        text,
                        line_start,
                        prev_break,
                        baseline,
                        height,
                        &mut cumulative_height,
                        &mut line_metrics,
                    );

                    line_start = prev_break;
                }
            }

            // now do the hard break
            add_line_metric(
                text,
                line_start,
                line_break,
                baseline,
                height,
                &mut cumulative_height,
                &mut line_metrics,
            );
            line_start = line_break;
            prev_break = line_break;
        }
    }

    line_metrics
}

fn add_line_metric(
    text: &str,
    start_offset: usize,
    end_offset: usize,
    baseline: f64,
    height: f64,
    cumulative_height: &mut f64,
    line_metrics: &mut Vec<LineMetric>,
) {
    *cumulative_height += height;

    let line = &text[start_offset..end_offset];
    let trailing_whitespace = count_trailing_whitespace(line);

    let line_metric = LineMetric {
        start_offset,
        end_offset,
        trailing_whitespace,
        baseline,
        height,
        cumulative_height: *cumulative_height,
    };
    line_metrics.push(line_metric);
}

// TODO: is non-breaking space trailing whitespace? Check with dwrite and
// coretext
fn count_trailing_whitespace(line: &str) -> usize {
    line.chars().rev().take_while(|c| c.is_whitespace()).count()
}

#[cfg(test)]
mod test {
    use super::super::*;
    use super::*;

    fn test_metrics_with_width(
        width: f64,
        expected: Vec<LineMetric>,
        input: &str,
        _text_layout: &mut CairoText, // actually not needed for test?
        font: &CairoFont,
    ) {
        let line_metrics = calculate_line_metrics(input, &font.0, width);

        for (i, (metric, exp)) in line_metrics.iter().zip(expected).enumerate() {
            println!("calculated: {:?}\nexpected: {:?}", metric, exp);

            assert_eq!(metric.start_offset, exp.start_offset);
            assert_eq!(metric.end_offset, exp.end_offset);
            assert_eq!(metric.trailing_whitespace, exp.trailing_whitespace);
            assert!(
                metric.cumulative_height < exp.cumulative_height + ((i as f64 + 1.0) * 3.0)
                    && metric.cumulative_height > exp.cumulative_height - ((i as f64 + 1.0) * 3.0)
            );
            assert!(metric.baseline < exp.baseline + 3.0 && metric.baseline > exp.baseline - 3.0);
            assert!(metric.height < exp.height + 3.0 && metric.height > exp.height - 3.0);
        }
    }

    #[test]
    fn test_hard_soft_break_end() {
        // This tests that the hard break is not handled before the soft break when the hard break
        // exceeds text layout width. In this case, it's the last line `best text!` which is too
        // long. The line should be soft-broken at the space before the EOL breaks.
        let input = "piet text is the best text!";
        let width = 50.0;

        let mut text = CairoText::new();
        let font = text.new_font_by_name("sans-serif", 12.0).build().unwrap();
        let line_metrics = calculate_line_metrics(input, &font.0, width);

        // Some print debugging, in case font size/width needs to be changed in future because of
        // brittle tests
        println!(
            "{}: \"piet text \"",
            font.0.text_extents("piet text ").x_advance
        );
        for lm in &line_metrics {
            let line_text = &input[lm.start_offset..lm.end_offset];
            println!(
                "{}: {:?}",
                font.0.text_extents(line_text).x_advance,
                line_text
            );
        }

        assert_eq!(line_metrics.len(), 5);
    }

    #[test]
    fn test_hard_soft_break_start() {
        // this tests that a single word followed by hard break that exceeds layout width is
        // correctly broken, and that there is no extra line metric created (e.g. a [0,0] line offset preceding)
        let input = "piet\ntext";
        let width = 10.0;

        let mut text = CairoText::new();
        let font = text.new_font_by_name("sans-serif", 12.0).build().unwrap();
        let line_metrics = calculate_line_metrics(input, &font.0, width);

        // Some print debugging, in case font size/width needs to be changed in future because of
        // brittle tests
        println!("{}: \"piet\n\"", font.0.text_extents("piet\n").x_advance);
        println!("{}: \"text\"", font.0.text_extents("text").x_advance);
        for lm in &line_metrics {
            let line_text = &input[lm.start_offset..lm.end_offset];
            println!(
                "{}: {:?}",
                font.0.text_extents(line_text).x_advance,
                line_text
            );
        }

        println!("line_metrics: {:?}", line_metrics);

        assert_eq!(line_metrics.len(), 2);
    }

    // TODO do a super-short length, to make sure the behavior is correct
    // when first break comes directly after the first word. I think I fixed it, but should have a
    // more explicit test.
    //
    // TODO add a macos specific test (I fudged this one to work for now)
    //
    // Test at three different widths: small, medium, large.
    // - small is every word being split.
    // - medium is one split.
    // - large is no split.
    //
    // Also test empty string input
    #[test]
    fn test_basic_calculate_line_metrics() {
        // Setup input, width, and expected
        let input = "piet text most best";

        use xi_unicode::LineBreakIterator;
        for (offset, line_break) in LineBreakIterator::new(input) {
            println!("{}:{}", offset, line_break);
        }

        let width_small = 30.0;
        let expected_small = vec![
            LineMetric {
                start_offset: 0,
                end_offset: 5,
                trailing_whitespace: 1,
                cumulative_height: 14.0,
                baseline: 12.0,
                height: 14.0,
            },
            LineMetric {
                start_offset: 5,
                end_offset: 10,
                trailing_whitespace: 1,
                cumulative_height: 28.0,
                baseline: 12.0,
                height: 14.0,
            },
            LineMetric {
                start_offset: 10,
                end_offset: 15,
                trailing_whitespace: 1,
                cumulative_height: 42.0,
                baseline: 12.0,
                height: 14.0,
            },
            LineMetric {
                start_offset: 15,
                end_offset: 19,
                trailing_whitespace: 0,
                cumulative_height: 56.0,
                baseline: 12.0,
                height: 14.0,
            },
        ];

        let width_medium = 70.0;
        let expected_medium = vec![
            LineMetric {
                start_offset: 0,
                end_offset: 10,
                trailing_whitespace: 1,
                cumulative_height: 14.0,
                baseline: 12.0,
                height: 14.0,
            },
            LineMetric {
                start_offset: 10,
                end_offset: 19,
                trailing_whitespace: 0,
                cumulative_height: 28.0,
                baseline: 12.0,
                height: 14.0,
            },
        ];

        let width_large = 125.0;
        let expected_large = vec![LineMetric {
            start_offset: 0,
            end_offset: 19,
            trailing_whitespace: 0,
            cumulative_height: 14.0,
            baseline: 12.0,
            height: 14.0,
        }];

        let empty_input = "";
        let expected_empty = vec![LineMetric {
            start_offset: 0,
            end_offset: 0,
            trailing_whitespace: 0,
            cumulative_height: 14.0,
            baseline: 12.0,
            height: 14.0,
        }];

        // setup cairo layout
        let mut text = CairoText::new();
        let font = text.new_font_by_name("sans-serif", 13.0).build().unwrap();

        println!(
            "piet text width: {}",
            font.0.text_extents("piet text").x_advance
        ); // 55
        println!(
            "most best width: {}",
            font.0.text_extents("most best").x_advance
        ); // 65
        println!(
            "piet text most best width: {}",
            font.0.text_extents("piet text most best").x_advance
        ); // 124

        test_metrics_with_width(width_small, expected_small, input, &mut text, &font);
        test_metrics_with_width(width_medium, expected_medium, input, &mut text, &font);
        test_metrics_with_width(width_large, expected_large, input, &mut text, &font);
        test_metrics_with_width(width_small, expected_empty, empty_input, &mut text, &font);
    }

    #[test]
    #[cfg(target_os = "linux")]
    // TODO determine if we need to test macos too for this. I don't think it's a big deal right
    // now, just wanted to make sure hard breaks work.
    fn test_basic_calculate_line_metrics_hard_break() {
        // Setup input, width, and expected
        let input = "piet\ntext most\nbest";

        use xi_unicode::LineBreakIterator;
        for (offset, line_break) in LineBreakIterator::new(input) {
            println!("{}:{}", offset, line_break);
        }

        let width_small = 25.0;
        let expected_small = vec![
            LineMetric {
                start_offset: 0,
                end_offset: 5,
                trailing_whitespace: 1,
                cumulative_height: 14.0,
                baseline: 12.0,
                height: 14.0,
            },
            LineMetric {
                start_offset: 5,
                end_offset: 10,
                trailing_whitespace: 1,
                cumulative_height: 28.0,
                baseline: 12.0,
                height: 14.0,
            },
            LineMetric {
                start_offset: 10,
                end_offset: 15,
                trailing_whitespace: 1,
                cumulative_height: 42.0,
                baseline: 12.0,
                height: 14.0,
            },
            LineMetric {
                start_offset: 15,
                end_offset: 19,
                trailing_whitespace: 0,
                cumulative_height: 56.0,
                baseline: 12.0,
                height: 14.0,
            },
        ];

        // setup cairo layout
        let mut text = CairoText::new();
        let font = text.new_font_by_name("sans-serif", 13.0).build().unwrap();

        test_metrics_with_width(width_small, expected_small, input, &mut text, &font);
    }

    #[test]
    fn test_count_trailing_whitespace() {
        assert_eq!(count_trailing_whitespace(" 1 "), 1);
        assert_eq!(count_trailing_whitespace(" 2  "), 2);
        assert_eq!(count_trailing_whitespace(" 3  \n"), 3);
    }
}
