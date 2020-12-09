
#![allow(warnings)] // TODO remove me!!!

use piet::kurbo::{Affine, PathEl, Point, QuadBez, Rect, Shape, Size};
use piet::{
    Color, Error, FixedGradient, FontFamily, HitTestPoint, ImageFormat, InterpolationMode,
    IntoBrush, LineMetric, RenderContext, StrokeStyle, Text, TextLayout, TextLayoutBuilder,
};
use std::borrow::Cow;
pub use text::*;
use skulpin::skia_safe;

mod text;

#[derive(Clone)]
pub enum Brush {
    Solid(u32),
}

impl<'a> IntoBrush<SkiaRenderContext<'a>> for Brush {
    fn make_brush<'b>(
        &'b self,
        _piet: &mut SkiaRenderContext,
        _bbox: impl FnOnce() -> Rect,
    ) -> std::borrow::Cow<'b, Brush> {
        Cow::Borrowed(self)
    }
}

pub struct SkiaRenderContext<'a> {
    canvas: &'a mut skia_safe::Canvas,
}

impl<'a> SkiaRenderContext<'a>{
    pub fn new(canvas: &'a mut skia_safe::Canvas) -> Self {
        SkiaRenderContext{
            canvas
        }
    }
}

pub struct SkiaImage;

impl<'a> RenderContext for SkiaRenderContext<'a> {
    type Brush = Brush;
    type Text = SkiaText;
    type TextLayout = SkiaTextLayout;
    type Image = SkiaImage;

    fn status(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn clear(&mut self, color: Color) {
        unimplemented!();
    }

    fn solid_brush(&mut self, color: Color) -> Brush {
        Brush::Solid(100)
        //unimplemented!();
    }

    fn gradient(&mut self, gradient: impl Into<FixedGradient>) -> Result<Brush, Error> {
        unimplemented!();
    }

    fn fill(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>) {
        dbg!("unimplemented");   
        //unimplemented!();
    }

    fn fill_even_odd(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>) {
        unimplemented!();
    }

    fn clip(&mut self, shape: impl Shape) {
        unimplemented!();
    }

    fn stroke(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>, width: f64) {
        dbg!("---");
        let f = 0.5;
        let mut paint = skia_safe::Paint::new(skia_safe::Color4f::new(1.0 - f, 0.0, f, 1.0), None);
        self.canvas.draw_line(
            skia_safe::Point::new(100.0, 500.0),
            skia_safe::Point::new(800.0, 500.0),
            &paint,
        );
    }

    fn stroke_styled(
        &mut self,
        shape: impl Shape,
        brush: &impl IntoBrush<Self>,
        width: f64,
        style: &StrokeStyle,
    ) {
        unimplemented!();
    }

    fn text(&mut self) -> &mut Self::Text {
        unimplemented!();
    }

    fn draw_text(&mut self, layout: &Self::TextLayout, pos: impl Into<Point>) {
        unimplemented!();
    }

    fn save(&mut self) -> Result<(), Error> {
        unimplemented!();
    }

    fn restore(&mut self) -> Result<(), Error> {
        unimplemented!();
    }

    fn finish(&mut self) -> Result<(), Error> {
        unimplemented!();
    }

    fn transform(&mut self, transform: Affine) {
        unimplemented!();
    }

    fn current_transform(&self) -> Affine {
        unimplemented!();
    }

    // allows e.g. raw_data[dst_off + x * 4 + 2] = buf[src_off + x * 4 + 0];
    #[allow(clippy::identity_op)]
    fn make_image(
        &mut self,
        width: usize,
        height: usize,
        buf: &[u8],
        format: ImageFormat,
    ) -> Result<Self::Image, Error> {
        unimplemented!();
    }

    #[inline]
    fn draw_image(
        &mut self,
        image: &Self::Image,
        dst_rect: impl Into<Rect>,
        interp: InterpolationMode,
    ) {
        unimplemented!();
    }

    #[inline]
    fn draw_image_area(
        &mut self,
        image: &Self::Image,
        src_rect: impl Into<Rect>,
        dst_rect: impl Into<Rect>,
        interp: InterpolationMode,
    ) {
        unimplemented!();
    }

    fn blurred_rect(&mut self, rect: Rect, blur_radius: f64, brush: &impl IntoBrush<Self>) {
        unimplemented!();
    }
}
