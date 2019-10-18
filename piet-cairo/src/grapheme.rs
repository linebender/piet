use piet::{
    HitTestPoint,
    TextLayout,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::CairoTextLayout;

// it's an inclusive range for both idx and x_point
// text_position is the index of the code point.
// This should all be utf8
impl CairoTextLayout {
    pub(crate) fn get_grapheme_boundaries(&self, text_position: u32) -> GraphemeBoundaries {
        let text = self.text.as_str();

        let mut res = GraphemeBoundaries::default();

        // First get the code unit boundaries, using unicode-segmentation
        // TODO need to a bounds check?
        let graphemes = UnicodeSegmentation::grapheme_indices(text, true);
        println!("{}: grapheme count:{}, bytes count: {}", text, graphemes.count(), text.len());

        let mut grapheme_indices = UnicodeSegmentation::grapheme_indices(text, true);
        grapheme_indices.by_ref().skip_while(|(i, _s)| *i < text_position as usize)
            .fold(0, |_,_| 0); // is this needed just to drive the skip?

        let leading_idx = grapheme_indices.next().unwrap().0;
        let trailing_idx = grapheme_indices.next().unwrap().0 - 1;

        // first do leading edge
        // if text position is 0, the leading edge defaults to 0
        if text_position != 0 {
            let previous_trailing_idx = leading_idx - 1; // looking at trailing edges only, not leading edge of current
            let previous_trailing_idx_trailing_hit = self.hit_test_text_position(previous_trailing_idx as u32, true)
                .expect("internal logic, code point not grapheme boundary");

            res.start_idx = leading_idx as u32;
            res.start_x = previous_trailing_idx_trailing_hit.point_x;
        }

        // now trailing edge
        let trailing_idx_trailing_hit = self.hit_test_text_position(trailing_idx as u32, true)
                .expect("internal logic, code point not grapheme boundary");

        res.end_idx = trailing_idx as u32;
        res.end_x = trailing_idx_trailing_hit.point_x;

        res
    }
}

pub fn point_x_in_grapheme(point_x: f32, grapheme_boundaries: &GraphemeBoundaries) -> Option<HitTestPoint> {
    let mut res = HitTestPoint::default();
    let start_x = grapheme_boundaries.start_x;
    let end_x = grapheme_boundaries.end_x;
    let start_idx = grapheme_boundaries.start_idx;
    let end_idx = grapheme_boundaries.end_idx;

    if point_x >= start_x && point_x <= end_x {
        // if inside, check which boundary it's closer to
        let is_trailing_hit = (end_x - point_x) > (point_x - start_x);

        res.is_inside= true;
        res.is_trailing_hit= is_trailing_hit; // TODO double check what this means?
        res.metrics.text_position = if is_trailing_hit { end_idx } else { start_idx };
        res.metrics.is_text = true;
        Some(res)
    } else {
        None
    }
}

#[derive(Debug, Default)]
pub struct GraphemeBoundaries {
    pub start_idx: u32,
    pub end_idx: u32,
    pub start_x: f32,
    pub end_x: f32,
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn test_hit_test_point() {
        let mut text_layout = CairoText::new();

        let font = text_layout.new_font_by_name("Segoe UI", 12.0).build().unwrap();
        let layout = text_layout.new_text_layout(&font, "piet text!").build().unwrap();

        // test hit test point
        let hit_test_point = layout.hit_test_point(19.0, 0.0);
        let hit_test_point_text_position = hit_test_point.metrics.text_position;
        println!("hit_test_point text_position: {}", hit_test_point_text_position);
    }
}
