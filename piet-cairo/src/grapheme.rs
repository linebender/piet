use piet::{
    HitTestPoint,
    TextLayout,
};

use crate::CairoTextLayout;

impl CairoTextLayout {
    pub(crate) fn get_grapheme_boundaries(&self, text_position: u32) -> Option<GraphemeBoundaries> {
        //  0 as default
        let mut res = GraphemeBoundaries::default();
        res.idx = text_position;

        let leading_edge = self.hit_test_text_position(text_position, false)?;
        let trailing_edge = self.hit_test_text_position(text_position, true)?;

        res.leading = leading_edge.point.x;
        res.trailing = trailing_edge.point.x;

        Some(res)
    }
}

pub fn point_x_in_grapheme(point_x: f64, grapheme_boundaries: &GraphemeBoundaries) -> Option<HitTestPoint> {
    let mut res = HitTestPoint::default();
    let leading = grapheme_boundaries.leading;
    let trailing = grapheme_boundaries.trailing;
    let idx = grapheme_boundaries.idx;

    if point_x >= leading && point_x <= trailing {
        // TODO if inside, check which boundary it's closer to
        res.is_trailing_hit= true;

        res.is_inside = true;
        res.metrics.text_position = idx;
        res.metrics.is_text = true;
        Some(res)
    } else {
        None
    }
}

#[derive(Debug, Default)]
pub struct GraphemeBoundaries {
    pub idx: u32,
    pub leading: f64,
    pub trailing: f64,
}

#[cfg(test)]
mod test {
    use crate::*;
    use piet::TextLayout;

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
