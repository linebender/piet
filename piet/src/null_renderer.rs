//! A render context that does nothing.

use alloc::borrow::Cow;
use core::ops::RangeBounds;

use kurbo::{Affine, Point, Rect, Shape, Size};

use crate::{
    Color, Error, FixedGradient, FontFamily, HitTestPoint, HitTestPosition, Image, ImageFormat,
    InterpolationMode, IntoBrush, LineMetric, RenderContext, StrokeStyle, Text, TextAttribute,
    TextLayout, TextLayoutBuilder, TextStorage,
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
#[derive(Clone)]
pub struct NullImage;

#[derive(Clone)]
#[doc(hidden)]
pub struct NullText;

#[doc(hidden)]
#[derive(Clone)]
pub struct NullTextLayout;
#[doc(hidden)]
pub struct NullTextLayoutBuilder;

impl NullRenderContext {
    #[allow(clippy::new_without_default)]
    #[doc(hidden)]
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

    fn clear(&mut self, _: impl Into<Option<Rect>>, _color: Color) {}

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

    fn draw_text(&mut self, _layout: &Self::TextLayout, _pos: impl Into<Point>) {}

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

    fn capture_image_area(&mut self, _src_rect: impl Into<Rect>) -> Result<Self::Image, Error> {
        Ok(NullImage)
    }

    fn make_image_with_stride(
        &mut self,
        _width: usize,
        _height: usize,
        _stride: usize,
        _buf: &[u8],
        _format: ImageFormat,
    ) -> Result<Self::Image, Error> {
        Ok(NullImage)
    }

    fn draw_image(
        &mut self,
        _image: &Self::Image,
        _dst_rect: impl Into<Rect>,
        _interp: InterpolationMode,
    ) {
    }
    fn draw_image_area(
        &mut self,
        _image: &Self::Image,
        _src_rect: impl Into<Rect>,
        _dst_rect: impl Into<Rect>,
        _interp: InterpolationMode,
    ) {
    }

    fn blurred_rect(&mut self, _rect: Rect, _blur_radius: f64, _brush: &impl IntoBrush<Self>) {}

    fn current_transform(&self) -> Affine {
        Affine::default()
    }
}

impl Text for NullText {
    type TextLayout = NullTextLayout;
    type TextLayoutBuilder = NullTextLayoutBuilder;

    fn load_font(&mut self, _data: &[u8]) -> Result<FontFamily, Error> {
        Ok(FontFamily::default())
    }

    fn new_text_layout(&mut self, _text: impl TextStorage) -> Self::TextLayoutBuilder {
        NullTextLayoutBuilder
    }

    fn font_family(&mut self, _family_name: &str) -> Option<FontFamily> {
        Some(FontFamily::default())
    }
}

impl TextLayoutBuilder for NullTextLayoutBuilder {
    type Out = NullTextLayout;

    fn max_width(self, _width: f64) -> Self {
        self
    }

    fn alignment(self, _alignment: crate::TextAlignment) -> Self {
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

    fn build(self) -> Result<Self::Out, Error> {
        Ok(NullTextLayout)
    }
}

impl TextLayout for NullTextLayout {
    fn size(&self) -> Size {
        Size::ZERO
    }

    fn trailing_whitespace_width(&self) -> f64 {
        0.0
    }

    fn image_bounds(&self) -> Rect {
        Rect::ZERO
    }

    fn line_text(&self, _line_number: usize) -> Option<&str> {
        None
    }

    fn line_metric(&self, _line_number: usize) -> Option<LineMetric> {
        None
    }

    fn line_count(&self) -> usize {
        0
    }

    fn hit_test_point(&self, _point: Point) -> HitTestPoint {
        HitTestPoint::default()
    }

    fn hit_test_text_position(&self, _text_position: usize) -> HitTestPosition {
        HitTestPosition::default()
    }

    fn text(&self) -> &str {
        ""
    }
}

impl IntoBrush<NullRenderContext> for NullBrush {
    fn make_brush<'b>(
        &'b self,
        _piet: &mut NullRenderContext,
        _bbox: impl FnOnce() -> Rect,
    ) -> Cow<'b, NullBrush> {
        Cow::Borrowed(self)
    }
}

impl Image for NullImage {
    fn size(&self) -> Size {
        Size::ZERO
    }
}
