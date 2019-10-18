//! Traits for fonts and text handling.

use crate::Error;

pub trait Text {
    type FontBuilder: FontBuilder<Out = Self::Font>;
    type Font: Font;

    type TextLayoutBuilder: TextLayoutBuilder<Out = Self::TextLayout>;
    type TextLayout: TextLayout;

    fn new_font_by_name(&mut self, name: &str, size: f64) -> Self::FontBuilder;

    fn new_text_layout(&mut self, font: &Self::Font, text: &str) -> Self::TextLayoutBuilder;
}

pub trait FontBuilder {
    type Out: Font;

    fn build(self) -> Result<Self::Out, Error>;
}

pub trait Font {}

pub trait TextLayoutBuilder {
    type Out: TextLayout;

    fn build(self) -> Result<Self::Out, Error>;
}

pub trait TextLayout {
    /// Measure the advance width of the text.
    fn width(&self) -> f64;

    /// Given a Point, determine the corresponding text position
    // TODO use kurbo::Point?
    fn hit_test_point(&self, point_x: f64, point_y: f64) -> HitTestPoint;

    /// Given a text position, determine the corresponding pixel location
    /// (In this case, the 
    /// (currently consider the text layout just one line)
    fn hit_test_text_position(&self, text_position: u32, trailing: bool) -> Option<HitTestTextPosition>;
}

#[derive(Debug, Default)]
pub struct HitTestPoint {
    pub metrics: HitTestMetrics,
    pub is_inside: bool,
    pub is_trailing_hit: bool,
}

#[derive(Debug, Default)]
pub struct HitTestTextPosition {
    pub point_x: f64,
    pub point_y: f64,
    pub metrics: HitTestMetrics,
}

#[derive(Debug, Default)]
pub struct HitTestMetrics {
    pub text_position: u32,
    //length: u32,
    //left: f32,
    //top: f32,
    //width: f32,
    //height: f32,
    //bidiLevel: u32,
    pub is_text: bool,
    //is_trimmed: bool,
}

