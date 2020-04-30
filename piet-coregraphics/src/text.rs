//! Text related stuff for the coregraphics backend

use piet::kurbo::Point;
use piet::{
    Error, Font, FontBuilder, HitTestPoint, HitTestTextPosition, LineMetric, Text, TextLayout,
    TextLayoutBuilder,
};

pub struct CoreGraphicsFont;

pub struct CoreGraphicsFontBuilder;

#[derive(Clone)]
pub struct CoreGraphicsTextLayout;

pub struct CoreGraphicsTextLayoutBuilder {}

pub struct CoreGraphicsText;

impl Text for CoreGraphicsText {
    type Font = CoreGraphicsFont;
    type FontBuilder = CoreGraphicsFontBuilder;
    type TextLayout = CoreGraphicsTextLayout;
    type TextLayoutBuilder = CoreGraphicsTextLayoutBuilder;

    fn new_font_by_name(&mut self, _name: &str, _size: f64) -> Self::FontBuilder {
        unimplemented!();
    }

    fn new_text_layout(
        &mut self,
        _font: &Self::Font,
        _text: &str,
        _width: impl Into<Option<f64>>,
    ) -> Self::TextLayoutBuilder {
        unimplemented!();
    }
}

impl Font for CoreGraphicsFont {}

impl FontBuilder for CoreGraphicsFontBuilder {
    type Out = CoreGraphicsFont;

    fn build(self) -> Result<Self::Out, Error> {
        unimplemented!();
    }
}

impl TextLayoutBuilder for CoreGraphicsTextLayoutBuilder {
    type Out = CoreGraphicsTextLayout;

    fn build(self) -> Result<Self::Out, Error> {
        unimplemented!()
    }
}

impl TextLayout for CoreGraphicsTextLayout {
    fn width(&self) -> f64 {
        0.0
    }

    fn update_width(&mut self, _new_width: impl Into<Option<f64>>) -> Result<(), Error> {
        unimplemented!()
    }

    fn line_text(&self, _line_number: usize) -> Option<&str> {
        unimplemented!()
    }

    fn line_metric(&self, _line_number: usize) -> Option<LineMetric> {
        unimplemented!()
    }

    fn line_count(&self) -> usize {
        unimplemented!()
    }

    fn hit_test_point(&self, _point: Point) -> HitTestPoint {
        unimplemented!()
    }

    fn hit_test_text_position(&self, _text_position: usize) -> Option<HitTestTextPosition> {
        unimplemented!()
    }
}
