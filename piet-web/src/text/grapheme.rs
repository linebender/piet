use piet::{HitTestPoint, TextLayout};
use unicode_segmentation::UnicodeSegmentation;

use crate::WebTextLayout;

// currently copied and pasted from cairo backend.
//
// However, not cleaning up because cairo and web implementations should diverge soon; and putting this
// code in `piet` core doesn't really make sense as it's implementation specific.
//
impl WebTextLayout {
    pub(crate) fn get_grapheme_boundaries(
        &self,
        grapheme_position: usize,
    ) -> Option<GraphemeBoundaries> {
        let mut graphemes = UnicodeSegmentation::grapheme_indices(self.text.as_str(), true);
        let (text_position, _) = graphemes.nth(grapheme_position)?;
        let (next_text_position, _) = graphemes.next().unwrap_or_else(|| (self.text.len(), ""));

        let curr_edge = self.hit_test_text_position(text_position)?;
        let next_edge = self.hit_test_text_position(next_text_position)?;

        let res = GraphemeBoundaries {
            curr_idx: curr_edge.metrics.text_position,
            next_idx: next_edge.metrics.text_position,
            leading: curr_edge.point.x,
            trailing: next_edge.point.x,
        };

        Some(res)
    }
}

pub(crate) fn point_x_in_grapheme(
    point_x: f64,
    grapheme_boundaries: &GraphemeBoundaries,
) -> Option<HitTestPoint> {
    let mut res = HitTestPoint::default();
    let leading = grapheme_boundaries.leading;
    let trailing = grapheme_boundaries.trailing;
    let curr_idx = grapheme_boundaries.curr_idx;
    let next_idx = grapheme_boundaries.next_idx;

    if point_x >= leading && point_x <= trailing {
        // Check which boundary it's closer to.
        // Round up to next grapheme boundary if
        let midpoint = leading + ((trailing - leading) / 2.0);
        if point_x >= midpoint {
            res.metrics.text_position = next_idx;
        } else {
            res.metrics.text_position = curr_idx;
        }

        res.is_inside = true;
        Some(res)
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
