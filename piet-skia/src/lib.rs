#![allow(warnings)] // TODO remove me!!!

use piet::kurbo::{Affine, PathEl, Point, Rect, Shape};
use piet::{
    Color, Error, FixedGradient, ImageFormat, InterpolationMode,
    IntoBrush, RenderContext, StrokeStyle, TextLayout, FixedLinearGradient,
    FixedRadialGradient,
};
use std::borrow::Cow;
pub use text::*;
use skia_safe;
use skia_safe::{Path, PaintStyle, Paint, FontMgr, TileMode};
use skia_safe::textlayout::{ParagraphBuilder, ParagraphStyle, FontCollection, TextStyle, Paragraph};
use skia_safe::effects::gradient_shader::{linear, radial};
use skia_safe::shader::Shader;
use skia_safe::ClipOp;

mod text;

fn pairf32(p: Point) -> (f32, f32) {
    (p.x as f32, p.y as f32)
}

#[derive(Clone)]
pub enum Brush {
    Solid(skia_safe::Color),
    Gradient(Shader)
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

fn apply_brush(paint: &mut Paint, brush: &Brush) {
    match brush {
        Brush::Solid(color) => {
            paint.set_color(*color);
        }
        Brush::Gradient(gradient) => {
            // clone might be inefficient
            paint.set_shader(gradient.clone());
        }
    }
}

// Convinience method for default Paint struct
// also possible to have single paint for all painting stuff
// but skia docs says that it's cheap to create
fn create_paint() -> Paint {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint
}

pub struct SkiaRenderContext<'a> {
    canvas: &'a mut skia_safe::Canvas,
}

impl<'a> SkiaRenderContext<'a>{
    pub fn new(canvas: &'a mut skia_safe::Canvas) -> Self {
        SkiaRenderContext{
            canvas,
        }
    }

    pub fn get_skia(&mut self) -> &mut skia_safe::Canvas {
        self.canvas
    }
}

pub struct SkiaImage;

// TODO this is temporal and we just want to use skia's paths
fn create_path(shape: impl Shape) -> Path {
    let mut path = Path::new();
    
    let mut last = Point::ZERO;
    for el in shape.path_elements(1e-3) {
        match el {
            PathEl::MoveTo(p) => {
                path.move_to(pairf32(p));
                last = p;
            }
            PathEl::LineTo(p) => {
                path.line_to(pairf32(p));
                last = p;
            }
            PathEl::QuadTo(p1, p2) => {
                path.quad_to(pairf32(p1), pairf32(p2));
                last = p2;
            }
            PathEl::CurveTo(p1, p2, p3) => {
                path.cubic_to(pairf32(p1), pairf32(p2), pairf32(p3));
                last = p3;
            }
            PathEl::ClosePath => {path.close();}
        }
    }
    path
}

pub fn convert_color(color: Color) -> skia_safe::Color {
    let rgba = color.as_rgba_u32();
    // swap r and a
    let argb = (rgba >> 8) | ((rgba & 255) << 24);
    skia_safe::Color::new(argb)
}

pub fn convert_point(point: Point) -> skia_safe::Point {
    skia_safe::Point::new(point.x as f32, point.y as f32)
}

impl<'a> RenderContext for SkiaRenderContext<'a> {
    type Brush = Brush;
    type Text = SkiaText;
    type TextLayout = SkiaTextLayout;
    type Image = SkiaImage;

    fn status(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn clear(&mut self, color: Color) {
        let color = convert_color(color);
        self.canvas.clear(color);
    }

    fn solid_brush(&mut self, color: Color) -> Brush {
        Brush::Solid(convert_color(color))
    }

    fn gradient(&mut self, gradient: impl Into<FixedGradient>) -> Result<Brush, Error> {
        // TODO
        let gradient = gradient.into();
        let mut colors_from_stops = |stops: Vec<piet::GradientStop>| {
            stops.into_iter().map(|stop| convert_color(stop.color)).collect()
        };
        let shader = match gradient {
            FixedGradient::Linear(FixedLinearGradient {
                start,
                end,
                stops
            }) => {
                let start = convert_point(start);
                let end = convert_point(end);
                let colors: Vec<_> = colors_from_stops(stops);
                linear((start, end), colors.as_slice(), None, TileMode::Clamp, None, None)
            }
            FixedGradient::Radial(FixedRadialGradient {
                center,
                origin_offset,
                radius,
                stops
            }) => {
                let center = convert_point(center);
                let radius = radius as f32;
                let colors: Vec<_> = colors_from_stops(stops);
                radial(center, radius, colors.as_slice(), None, TileMode::Clamp, None, None)
            }
        };
        Ok(Brush::Gradient(shader.unwrap()))
    }

    fn fill(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        let mut paint = create_paint();
        apply_brush(&mut paint, brush.as_ref());
        let mut path = create_path(shape);
        self.canvas.draw_path(&path, &paint);
    }

    fn fill_even_odd(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>) {}

    fn clip(&mut self, shape: impl Shape) {
        let mut path = create_path(shape);
        self.canvas.clip_path(&path, ClipOp::Intersect, false);
    }

    fn stroke(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>, width: f64) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        let mut paint = create_paint();
        apply_brush(&mut paint, brush.as_ref());
        paint.set_stroke_width(width as f32);
        paint.set_style(PaintStyle::Stroke);
        let mut path = create_path(shape);
        self.canvas.draw_path(&path, &paint);
    }

    fn stroke_styled(
        &mut self,
        shape: impl Shape,
        brush: &impl IntoBrush<Self>,
        width: f64,
        style: &StrokeStyle,
    ) {
        //unimplemented!();
    }

    fn text(&mut self) -> &mut Self::Text {
        unimplemented!();
    }

    fn draw_text(&mut self, layout: &Self::TextLayout, pos: impl Into<Point>) {
        let pos = pos.into();
        let mut paint = create_paint();
        let rect = layout.image_bounds() + pos.to_vec2();
        let brush = layout.fg_color.make_brush(self, || rect);
        let mut paint = create_paint();
        apply_brush(&mut paint, brush.as_ref());
        
        let pos = skia_safe::Point::new(pos.x as f32, pos.y as f32);
        layout.paragraph.paint(&mut self.canvas, pos); 
    }

    fn save(&mut self) -> Result<(), Error> {
        self.canvas.save();
        return Ok(())
    }

    fn restore(&mut self) -> Result<(), Error> {
        self.canvas.restore();
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Error> {
        unimplemented!();
    }

    fn transform(&mut self, transform: Affine) {
        let coefs = transform.as_coeffs();
        let mut matrix = [0f32; 6];
        for (e, c) in matrix.iter_mut().zip(coefs.iter()) {
            *e = *c as f32;
        };
        let matrix = skia_safe::Matrix::from_affine(&matrix);
        self.canvas.concat(&matrix);
    }

    fn current_transform(&self) -> Affine {
        // TODO figure out why anim.rs example is not working
        //let matrix = self.canvas.total_matrix();
        //if let Some(affine) = matrix.to_affine() {
        //    let mut matrix = [0f64; 6];
        //    for (e, c) in matrix.iter_mut().zip(affine.iter()) {
        //        *e = *c as f64;
        //    }
        //    Affine::new(matrix)
        //} else {
        //    Affine::new([1., 0., 0., 1., 0., 0.])
        //};
        Affine::new([1., 0., 0., 1., 0., 0.])
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
