// Copyright 2019 the Piet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use piet::HitTestPoint;
use unicode_segmentation::UnicodeSegmentation;
use web_sys::CanvasRenderingContext2d;

use super::hit_test_line_position;

// currently copied and pasted from cairo backend.
//
// However, not cleaning up because cairo and web implementations should diverge soon; and putting this
// code in `piet` core doesn't really make sense as it's implementation specific.
//
/// get grapheme boundaries, intended to act on a line of text, not a full text layout that has
/// both horizontal and vertical components
pub(crate) fn get_grapheme_boundaries(
    ctx: &CanvasRenderingContext2d,
    text: &str,
    grapheme_position: usize,
) -> Option<GraphemeBoundaries> {
    let mut graphemes = UnicodeSegmentation::grapheme_indices(text, true);
    let (text_position, _) = graphemes.nth(grapheme_position)?;
    let (next_text_position, _) = graphemes.next().unwrap_or((text.len(), ""));

    let curr_edge = hit_test_line_position(ctx, text, text_position);
    let next_edge = hit_test_line_position(ctx, text, next_text_position);

    let res = GraphemeBoundaries {
        curr_idx: text_position,
        next_idx: next_text_position,
        leading: curr_edge,
        trailing: next_edge,
    };

    Some(res)
}

pub(crate) fn point_x_in_grapheme(
    point_x: f64,
    grapheme_boundaries: &GraphemeBoundaries,
) -> Option<HitTestPoint> {
    let leading = grapheme_boundaries.leading;
    let trailing = grapheme_boundaries.trailing;
    let curr_idx = grapheme_boundaries.curr_idx;
    let next_idx = grapheme_boundaries.next_idx;

    if point_x >= leading && point_x <= trailing {
        // Check which boundary it's closer to.
        // Round up to next grapheme boundary if
        let midpoint = leading + ((trailing - leading) / 2.0);
        let is_inside = true;
        let idx = if point_x >= midpoint {
            next_idx
        } else {
            curr_idx
        };
        Some(HitTestPoint::new(idx, is_inside))
    } else {
        None
    }
}

#[derive(Debug, Default, PartialEq)]
pub(crate) struct GraphemeBoundaries {
    pub curr_idx: usize,
    pub next_idx: usize,
    pub leading: f64,
    // not technically trailing; it's the lead boundary for the next grapheme cluster
    pub trailing: f64,
}
