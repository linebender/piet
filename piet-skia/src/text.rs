use std::ops::RangeBounds;

use piet::kurbo::{Point, Rect, Size};
use piet::{
    util, Color, Error, FontFamily, FontStyle, HitTestPoint, HitTestPosition, LineMetric, Text,
    TextAttribute, TextLayout, TextLayoutBuilder, TextStorage,
};

#[derive(Clone)]
pub struct SkiaText {}

impl SkiaText {
    pub fn new() -> Self {
        SkiaText{}
    }
}

#[derive(Clone)]
pub struct SkiaTextLayout {}

pub struct SkiaTextLayoutBuilder {}

impl Text for SkiaText {
    type TextLayout = SkiaTextLayout;
    type TextLayoutBuilder = SkiaTextLayoutBuilder;

    fn font_family(&mut self, family_name: &str) -> Option<FontFamily> {
        Some(FontFamily::new_unchecked(family_name))
    }

    fn load_font(&mut self, _data: &[u8]) -> Result<FontFamily, Error> {
        Err(Error::NotSupported)
    }

    fn new_text_layout(&mut self, text: impl TextStorage) -> Self::TextLayoutBuilder {
        unimplemented!();
    }
}

impl TextLayoutBuilder for SkiaTextLayoutBuilder {
    type Out = SkiaTextLayout;

    fn max_width(mut self, width: f64) -> Self {
        unimplemented!();
    }

    fn alignment(self, _alignment: piet::TextAlignment) -> Self {
        unimplemented!();
    }

    fn default_attribute(mut self, attribute: impl Into<TextAttribute>) -> Self {
        unimplemented!();
    }

    fn range_attribute(
        self,
        _range: impl RangeBounds<usize>,
        _attribute: impl Into<TextAttribute>,
    ) -> Self {
        unimplemented!();
    }

    fn build(self) -> Result<Self::Out, Error> {
        unimplemented!();
    }
}

impl TextLayout for SkiaTextLayout {
    fn size(&self) -> Size {
        unimplemented!();
    }

    fn trailing_whitespace_width(&self) -> f64 {
        unimplemented!();
    }

    fn image_bounds(&self) -> Rect {
        unimplemented!();
    }

    fn text(&self) -> &str {
        unimplemented!();
    }

    fn line_text(&self, line_number: usize) -> Option<&str> {
        unimplemented!();
    }

    fn line_metric(&self, line_number: usize) -> Option<LineMetric> {
        unimplemented!();
    }

    fn line_count(&self) -> usize {
        unimplemented!();
    }

    fn hit_test_point(&self, point: Point) -> HitTestPoint {
        unimplemented!();
    }

    fn hit_test_text_position(&self, idx: usize) -> HitTestPosition {
        unimplemented!();
    }
}
