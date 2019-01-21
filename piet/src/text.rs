//! Traits for fonts and text handling.

use crate::{Error, RoundFrom, RoundInto};

pub trait Text {
    type FontBuilder: FontBuilder<Out = Self::Font>;
    type Font: Font;

    type TextLayoutBuilder: TextLayoutBuilder<Out = Self::TextLayout>;
    type TextLayout: TextLayout;

    type Coord: Into<f64> + RoundFrom<f64>;

    fn new_font_by_name(
        &mut self,
        name: &str,
        size: impl RoundInto<Self::Coord>,
    ) -> Result<Self::FontBuilder, Error>;

    fn new_text_layout(
        &mut self,
        font: &Self::Font,
        text: &str,
    ) -> Result<Self::TextLayoutBuilder, Error>;
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
    type Coord: Into<f64> + RoundFrom<f64>;

    /// Measure the advance width of the text.
    fn width(&self) -> Self::Coord;
}
