use piet::{HitTestPoint, TextLayout};
use unicode_segmentation::UnicodeSegmentation;

use crate::CairoTextLayout;

impl CairoTextLayout {
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
#[cfg(test)]
mod test {
    use super::*;
    use crate::*;

    #[test]
    fn test_grapheme_boundaries() {
        let mut text_layout = CairoText::new();

        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout.new_text_layout(&font, "piet").build().unwrap();

        let expected_3 = GraphemeBoundaries {
            curr_idx: 3,
            next_idx: 4,
            leading: 0.0,  // not testing x advance
            trailing: 0.0, // not testing x advance
        };

        // test grapheme boundaries
        assert_eq!(
            layout.get_grapheme_boundaries(3).unwrap().curr_idx,
            expected_3.curr_idx
        );
        assert_eq!(
            layout.get_grapheme_boundaries(3).unwrap().next_idx,
            expected_3.next_idx
        );
        assert_eq!(layout.get_grapheme_boundaries(4), None);
    }

    #[test]
    fn test_x_in_grapheme_boundaries() {
        let bounds = GraphemeBoundaries {
            curr_idx: 2,
            next_idx: 4,
            leading: 10.0,
            trailing: 14.0,
        };

        let expected_curr = Some(HitTestPoint {
            metrics: HitTestMetrics {
                text_position: 2,
                ..Default::default()
            },
            is_inside: true,
            ..Default::default()
        });
        let expected_next = Some(HitTestPoint {
            metrics: HitTestMetrics {
                text_position: 4,
                ..Default::default()
            },
            is_inside: true,
            ..Default::default()
        });

        assert_eq!(point_x_in_grapheme(10.0, &bounds), expected_curr);
        assert_eq!(point_x_in_grapheme(11.0, &bounds), expected_curr);
        assert_eq!(point_x_in_grapheme(12.0, &bounds), expected_next);
        assert_eq!(point_x_in_grapheme(13.0, &bounds), expected_next);
    }
}
