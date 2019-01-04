//! Traits for fonts and text handling.

pub trait FontBuilder {
    type Out: Font;

    fn build(self) -> Self::Out;
}

pub trait Font {}

pub trait TextLayoutBuilder {
    type Out: TextLayout;

    fn build(self) -> Self::Out;
}

pub trait TextLayout {}
