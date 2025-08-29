// Copyright 2020 the Piet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

// currently basically copied and pasted from cairo backend, except cairo::ScaledFont is replaced
// by web_sys::CanvasRenderingContext2d
//
// However, not cleaning up because cairo and web implementations should diverge soon; and putting this
// code in `piet` core doesn't really make sense as it's implementation specific.
//

use web_sys::CanvasRenderingContext2d;
use xi_unicode::LineBreakIterator;

use super::{LineMetric, text_width};

// NOTE font_size is used only for heuristic purposes, prefer actual web-api for height and
// baseline when available.
#[allow(clippy::branches_sharing_code)] // clearer as written
pub(crate) fn calculate_line_metrics(
    text: &str,
    ctx: &CanvasRenderingContext2d,
    width: f64,
    font_size: f64,
) -> Vec<LineMetric> {
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
    let mut y_offset = 0.0;

    // Vertical measures constant across all lines for now (web text)
    // We use heuristics because we don't have access to web apis through web-sys yet.
    let height = font_size * 1.2;
    let baseline = height * 0.8;

    for (line_break, is_hard_break) in LineBreakIterator::new(text) {
        if !is_hard_break {
            // this section is for soft breaks
            let curr_str = &text[line_start..line_break];
            let curr_width = text_width(curr_str, ctx);

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
                    &mut y_offset,
                    &mut line_metrics,
                );

                // Now handle the graphemes between prev_break and current break. The
                // implementation depends on how we're treating a single line that's wider than
                // desired width. For now, just assume that the word will get cutoff when rendered.
                //
                // If it's shorter than desired width, just continue.

                let curr_str = &text[prev_break..line_break];
                let curr_width = text_width(curr_str, ctx);

                if curr_width > width {
                    add_line_metric(
                        text,
                        prev_break,
                        line_break,
                        baseline,
                        height,
                        &mut y_offset,
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
            let curr_width = text_width(curr_str, ctx);

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
                        &mut y_offset,
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
                &mut y_offset,
                &mut line_metrics,
            );
            line_start = line_break;
            prev_break = line_break;
        }
    }

    // the trailing line, if there is no explicit newline.
    if line_start != text.len() {
        add_line_metric(
            text,
            line_start,
            text.len(),
            baseline,
            height,
            &mut y_offset,
            &mut line_metrics,
        );
    }

    line_metrics
}

fn add_line_metric(
    text: &str,
    start_offset: usize,
    end_offset: usize,
    baseline: f64,
    height: f64,
    y_offset: &mut f64,
    line_metrics: &mut Vec<LineMetric>,
) {
    let line = &text[start_offset..end_offset];
    let trailing_whitespace = count_trailing_whitespace(line);

    let line_metric = LineMetric {
        start_offset,
        end_offset,
        trailing_whitespace,
        baseline,
        height,
        y_offset: *y_offset,
    };
    line_metrics.push(line_metric);
    *y_offset += height;
}

// TODO: is non-breaking space trailing whitespace? Check with dwrite and
// coretext
fn count_trailing_whitespace(line: &str) -> usize {
    line.chars()
        .rev()
        .take_while(|c| c.is_whitespace())
        .map(char::len_utf8)
        .sum()
}
