//! Text functionality for Piet svg backend

use std::ops::RangeBounds;

use piet::kurbo::{Point, Rect, Size};
use piet::{
    Error, FontFamily, HitTestPoint, HitTestPosition, LineMetric, TextAttribute, TextStorage,
};

type Result<T> = std::result::Result<T, Error>;

/// SVG text (unimplemented)
#[derive(Debug, Clone)]
pub struct Text;

impl Text {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Text
    }
}

impl piet::Text for Text {
    type TextLayout = TextLayout;
    type TextLayoutBuilder = TextLayoutBuilder;

    fn font_family(&mut self, _family_name: &str) -> Option<FontFamily> {
        Some(FontFamily::default())
    }

    fn load_font(&mut self, _data: &[u8]) -> Result<FontFamily> {
        Ok(FontFamily::default())
    }

    fn new_text_layout(&mut self, _text: impl TextStorage) -> TextLayoutBuilder {
        TextLayoutBuilder
    }
}

#[derive(Debug)]
pub struct TextLayoutBuilder;

impl piet::TextLayoutBuilder for TextLayoutBuilder {
    type Out = TextLayout;

    fn max_width(self, _width: f64) -> Self {
        self
    }

    fn alignment(self, _alignment: piet::TextAlignment) -> Self {
        self
    }

    fn default_attribute(self, _attribute: impl Into<TextAttribute>) -> Self {
        self
    }

    fn range_attribute(
        self,
        _range: impl RangeBounds<usize>,
        _attribute: impl Into<TextAttribute>,
    ) -> Self {
        self
    }

    fn build(self) -> Result<TextLayout> {
        Err(Error::NotSupported)
    }
}

/// SVG text layout (unimplemented)
#[derive(Debug, Clone)]
pub struct TextLayout;

impl piet::TextLayout for TextLayout {
    fn size(&self) -> Size {
        unimplemented!()
    }

    fn trailing_whitespace_width(&self) -> f64 {
        unimplemented!()
    }

    fn image_bounds(&self) -> Rect {
        unimplemented!()
    }

    #[allow(clippy::unimplemented)]
    fn line_text(&self, _line_number: usize) -> Option<&str> {
        unimplemented!();
    }

    #[allow(clippy::unimplemented)]
    fn line_metric(&self, _line_number: usize) -> Option<LineMetric> {
        unimplemented!();
    }

    #[allow(clippy::unimplemented)]
    fn line_count(&self) -> usize {
        unimplemented!();
    }

    fn hit_test_point(&self, _point: Point) -> HitTestPoint {
        unimplemented!()
    }

    fn hit_test_text_position(&self, _text_position: usize) -> HitTestPosition {
        unimplemented!()
    }

    fn text(&self) -> &str {
        unimplemented!()
    }
}
