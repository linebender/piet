//! Traits for fonts and text handling.

use crate::RoundFrom;

pub trait FontBuilder {
    type Out: Font;

    // TODO: this should probably give a Result, it could fail.
    fn build(self) -> Self::Out;
}

pub trait Font {}

pub trait TextLayoutBuilder {
    type Out: TextLayout;

    // TODO: this should probably give a Result, it could fail.
    fn build(self) -> Self::Out;
}

pub trait TextLayout {
    type Coord: Into<f64> + RoundFrom<f64>;

    /// Measure the advance width of the text.
    fn width(&self) -> Self::Coord;
}
