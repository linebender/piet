use piet::{
    HitTestPoint,
    TextLayout,
};

use crate::CairoTextLayout;

// it's an inclusive range for both idx and x_point
// text_position is the index of the grapheme.
// Hit test text position deals with graphemes, so can just call it.
impl CairoTextLayout {
    pub(crate) fn get_grapheme_boundaries(&self, text_position: u32) -> GraphemeBoundaries {
        //  0 as default
        let mut res = GraphemeBoundaries::default();

        // leading edge
        if text_position != 0 {
            let leading_idx = (text_position - 1) as u32;
            let leading_edge = self.hit_test_text_position(leading_idx, true)
                    .expect("internal logic, code point not grapheme boundary");

            res.start_idx = leading_idx;
            res.start_x = leading_edge.point_x;
        }

        // now trailing edge
        let trailing_edge = self.hit_test_text_position(text_position as u32, true)
                .expect("internal logic, code point not grapheme boundary");

        res.end_idx = text_position as u32;
        res.end_x = trailing_edge.point_x;

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
    use piet::TextLayout;

    #[test]
    #[ignore]
    fn test_hit_test_point() {
        let mut text_layout = CairoText::new();

        let font = text_layout.new_font_by_name("Segoe UI", 12.0).build().unwrap();
        let layout = text_layout.new_text_layout(&font, "piet text!").build().unwrap();
        println!("text width: {}", layout.width());
        println!("text width: {:?}", layout.hit_test_text_position(3, true));
        println!("text width: {:?}", layout.hit_test_text_position(4, true));

        // test hit test point
        let hit_test_point = layout.hit_test_point(19.0, 0.0);
        let hit_test_point_text_position = hit_test_point.metrics.text_position;
        println!("hit_test_point text_position: {}", hit_test_point_text_position);
    }

    #[test]
    fn test_grapheme_boundaries() {
        let mut text_layout = CairoText::new();

        let font = text_layout.new_font_by_name("Segoe UI", 12.0).build().unwrap();
        let layout = text_layout.new_text_layout(&font, "piet text!").build().unwrap();
        println!("text width: {}", layout.width());
        println!("position 3 trailing pt: {:?}", layout.hit_test_text_position(3, true));
        println!("position 4 trailing pt: {:?}", layout.hit_test_text_position(4, true));

        // test grapheme boundaries
        let grapheme_boundaries = layout.get_grapheme_boundaries(4);
        println!("grapheme boundaries: {:?}", grapheme_boundaries);
    }
}
