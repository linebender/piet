//! Traits for fonts and text handling.

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

pub trait TextLayout {}
