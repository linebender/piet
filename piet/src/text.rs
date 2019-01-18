//! Traits for fonts and text handling.

use crate::{Error, RoundFrom};

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
