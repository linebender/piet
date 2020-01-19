//! A render context that does nothing.

use std::borrow::Cow;

use kurbo::{Affine, Point, Rect, Shape};

use crate::{
    Color, Error, FixedGradient, Font, FontBuilder, HitTestPoint, HitTestTextPosition, ImageFormat,
    InterpolationMode, IntoBrush, RenderContext, StrokeStyle, Text, TextLayout, TextLayoutBuilder,
};

/// A render context that doesn't render.
///
/// This is useful largely for doc tests, but is made public in case
/// it might come in handy.
#[doc(hidden)]
pub struct NullRenderContext(NullText);

#[derive(Clone)]
#[doc(hidden)]
pub struct NullBrush;
#[doc(hidden)]
pub struct NullImage;

#[doc(hidden)]
pub struct NullText;

#[doc(hidden)]
pub struct NullFont;
#[doc(hidden)]
pub struct NullFontBuilder;

#[doc(hidden)]
pub struct NullTextLayout;
#[doc(hidden)]
pub struct NullTextLayoutBuilder;

impl NullRenderContext {
    pub fn new() -> NullRenderContext {
        NullRenderContext(NullText)
    }
}

impl RenderContext for NullRenderContext {
    type Brush = NullBrush;
    type Image = NullImage;
    type Text = NullText;
    type TextLayout = NullTextLayout;

    fn status(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn solid_brush(&mut self, _color: Color) -> Self::Brush {
        NullBrush
    }

    fn gradient(&mut self, _gradient: impl Into<FixedGradient>) -> Result<Self::Brush, Error> {
        Ok(NullBrush)
    }

    fn clear(&mut self, _color: Color) {}

    fn stroke(&mut self, _shape: impl Shape, _brush: &impl IntoBrush<Self>, _width: f64) {}

    fn stroke_styled(
        &mut self,
        _shape: impl Shape,
        _brush: &impl IntoBrush<Self>,
        _width: f64,
        _style: &StrokeStyle,
    ) {
    }

    fn fill(&mut self, _shape: impl Shape, _brush: &impl IntoBrush<Self>) {}

    fn fill_even_odd(&mut self, _shape: impl Shape, _brush: &impl IntoBrush<Self>) {}

    fn clip(&mut self, _shape: impl Shape) {}

    fn text(&mut self) -> &mut Self::Text {
        &mut self.0
    }

    fn draw_text(
        &mut self,
        _layout: &Self::TextLayout,
        _pos: impl Into<Point>,
        _brush: &impl IntoBrush<Self>,
    ) {
    }

    fn save(&mut self) -> Result<(), Error> {
        Ok(())
    }
    fn restore(&mut self) -> Result<(), Error> {
        Ok(())
    }
    fn finish(&mut self) -> Result<(), Error> {
        Ok(())
    }
    fn transform(&mut self, _transform: Affine) {}

    fn make_image(
        &mut self,
        _width: usize,
        _height: usize,
        _buf: &[u8],
        _format: ImageFormat,
    ) -> Result<Self::Image, Error> {
        Ok(NullImage)
    }
    fn draw_image(
        &mut self,
        _image: &Self::Image,
        _rect: impl Into<Rect>,
        _interp: InterpolationMode,
    ) {
    }

    fn current_transform(&self) -> Affine {
        Affine::default()
    }
}

impl Text for NullText {
    type Font = NullFont;
    type FontBuilder = NullFontBuilder;
    type TextLayout = NullTextLayout;
    type TextLayoutBuilder = NullTextLayoutBuilder;

    fn new_font_by_name(&mut self, _name: &str, _size: f64) -> Self::FontBuilder {
        NullFontBuilder
    }

    fn new_text_layout(&mut self, _font: &Self::Font, _text: &str) -> Self::TextLayoutBuilder {
        NullTextLayoutBuilder
    }
}

impl Font for NullFont {}

impl FontBuilder for NullFontBuilder {
    type Out = NullFont;

    fn build(self) -> Result<Self::Out, Error> {
        Ok(NullFont)
    }
}

impl TextLayoutBuilder for NullTextLayoutBuilder {
    type Out = NullTextLayout;

    fn build(self) -> Result<Self::Out, Error> {
        Ok(NullTextLayout)
    }
}

impl TextLayout for NullTextLayout {
    fn width(&self) -> f64 {
        42.0
    }

    fn hit_test_point(&self, _point: Point) -> HitTestPoint {
        HitTestPoint::default()
    }

    fn hit_test_text_position(&self, _text_position: usize) -> Option<HitTestTextPosition> {
        None
    }
}

impl IntoBrush<NullRenderContext> for NullBrush {
    fn make_brush<'b>(
        &'b self,
        _piet: &mut NullRenderContext,
        _bbox: impl FnOnce() -> Rect,
    ) -> std::borrow::Cow<'b, NullBrush> {
        Cow::Borrowed(self)
    }
}
