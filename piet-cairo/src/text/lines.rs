use xi_unicode::LineBreakIterator;

use super::{CairoFont, LineMetric};

pub(crate) fn calculate_line_metrics(text: &str, font: &CairoFont, width: f64) -> Vec<LineMetric> {
    // first pass, completely naive and inefficient. Check at every break to see if line longer
    // than width.
    //
    // See https://raphlinus.github.io/rust/skribo/text/2019/04/26/skribo-progress.html for
    // some other ideas for better efficiency
    //
    // NOtes:
    // hard breaks: cr, lf, line break, para char. Mandated by unicode
    //
    // So, every time there's a a hard break, must break.
    //
    // soft-hyphen, don't need to deal with this round. Looks like automatic hyphenation, but
    // with unicode codepoint. Don't special case, let if break for now.
    //
    // For soft breaks, then I need to check line widths etc.
    //
    //
    // - TODO what happens when even smallest break is wider than width?
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
    let mut cum_height = 0.0;

    for (line_break, is_hard_break) in LineBreakIterator::new(text) {
        if is_hard_break {
            let curr_str = &text[line_start..line_break];
            let curr_width = font.0.text_extents(curr_str).x_advance;

            if curr_width > width {
                // if current line_break is too wide, use the prev break
                // TODO how to get trailing_whitespace? Do I need to count whitespace units
                // backwards from the last break?
                // Note: Yes, use chars().rev().while(|c| c.is_whitespace()).count()
                // Note: is non-breaking space trailing whitespace? Check with dwrite and
                // coretext

                // >===================================================
                // first do the line to prev break

                let height = font.0.extents().height;
                cum_height += height;

                let baseline = font.0.extents().ascent;

                let line_metric = LineMetric {
                    start_offset: line_start,
                    end_offset: prev_break,
                    trailing_whitespace: 0,
                    baseline,
                    height,
                    cumulative_height: cum_height,
                };
                line_metrics.push(line_metric);
                // <===================================================

                // >===================================================
                // Now handle the graphemes between prev_break and current break
                // reset line state
                line_start = prev_break;

                let curr_str = &text[line_start..line_break];
                let curr_width = font.0.text_extents(curr_str).x_advance;

                if curr_width < width {
                    let height = font.0.extents().height;
                    cum_height += height;

                    let baseline = font.0.extents().ascent;

                    let line_metric = LineMetric {
                        start_offset: line_start,
                        end_offset: prev_break,
                        trailing_whitespace: 0,
                        baseline,
                        height,
                        cumulative_height: cum_height,
                    };
                    line_metrics.push(line_metric);

                    line_start = line_break;
                    prev_break = line_start;
                } else {
                    continue;
                }
                // <===================================================
            }
        } else {
            prev_break = line_break;
            continue;
        }
    }

    line_metrics
}
