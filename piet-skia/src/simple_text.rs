use std::rc::Rc;

use piet::kurbo::{Point, Rect, Size};
use piet::{
    util, Color, HitTestPoint, HitTestPosition, LineMetric,
    TextLayout, TextStorage,
};
use skia_safe::Font;
use xi_unicode::LineBreakIterator;
use std::fmt;

#[derive(Debug, Clone)]
pub struct SimpleLineMetric {
    pub start_offset: usize,
    pub end_offset: usize,
    pub y_offset: f64,
    pub bounds: skia_safe::Rect
}

fn add_line_metric(
    text: &str,
    start_offset: usize,
    end_offset: usize,
    y_offset: &mut f64,
    line_metrics: &mut Vec<SimpleLineMetric>,
    font: &Font,
) {
    let line = &text[start_offset..end_offset];
   
    let (_width, bounds) = font.measure_str(line, None);
    let line_metric = SimpleLineMetric {
        start_offset,
        end_offset,
        y_offset: *y_offset,
        bounds
    };
    line_metrics.push(line_metric);
    *y_offset += bounds.height() as f64;
}

pub(crate) fn calculate_line_metrics(text: &str, font: &Font) -> Vec<SimpleLineMetric> {
    let mut line_metrics = Vec::new();
    let mut line_start = 0;
    let mut y_offset = 0.0;
    for (line_break, is_hard_break) in LineBreakIterator::new(text) {
        if is_hard_break { 
            add_line_metric(
                text,
                line_start,
                line_break,
                &mut y_offset,
                &mut line_metrics,
                font
            );
            line_start = line_break;

        }
    }
    // the trailing line, if there is no explicit newline.
    if line_start != text.len() {
        add_line_metric(
            text,
            line_start,
            text.len(),
            &mut y_offset,
            &mut line_metrics,
            font
        );
    }
    line_metrics
}

#[derive(Clone)]
pub struct SkiaSimpleText;

#[derive(Clone)]
pub struct SkiaSimpleTextLayout {
    // culculated on build
    pub(crate) line_metrics: Vec<SimpleLineMetric>,
    pub(crate) fg_color: Color,
    pub(crate) size: Size,
    // skia doesn't support Clone trait for font
    pub font: Rc<Font>,
    pub text: Rc<dyn TextStorage>,

}

impl fmt::Debug for SkiaSimpleTextLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SkiaTextLayoutBuilder")
            .field("fg_color", &self.fg_color)
            .field("font", &self.font)
            .field("text", &self.text.as_str())
            .finish()
    }
}

impl TextLayout for SkiaSimpleTextLayout {
    fn size(&self) -> Size {
        self.size
    }

    fn trailing_whitespace_width(&self) -> f64 {
        unimplemented!();
    }

    fn image_bounds(&self) -> Rect {
        self.size.to_rect()
    }

    fn text(&self) -> &str {
        &self.text
    }

    fn line_text(&self, line_number: usize) -> Option<&str> {
        unimplemented!();
    }

    fn line_metric(&self, line_number: usize) -> Option<LineMetric> {
        // for now we can just support only one line text 
        let mut metrics = LineMetric::default();
        Some(metrics) // TODO
    }

    fn line_count(&self) -> usize {
        unimplemented!();
    }

    fn hit_test_point(&self, point: Point) -> HitTestPoint { 
        // TODO
        HitTestPoint::new(0, false)
        //if point.y > self.paragraph.height() {
        //   return HitTestPoint::default() 
        //}
        //let width = self.paragraph
        //    .get_line_metrics()
        //    .iter()
        //    .map(|l| l.width);
    }

    fn hit_test_text_position(&self, idx: usize) -> HitTestPosition {
        // TODO
        HitTestPosition::new(Point::new(0., 0.), 0)
    }
}
