use std::fmt;

use piet::kurbo::{Affine, PathEl, Point, Rect, Shape, Size};
use piet::{
    Color, Error, FixedGradient, ImageFormat, InterpolationMode,
    IntoBrush, RenderContext, StrokeStyle, TextLayout, FixedLinearGradient,
    FixedRadialGradient, Image, LineJoin, LineCap
};
use std::borrow::Cow;
pub use text::*;
use skia_safe;
use skia_safe::{Path, PaintStyle, Paint, TileMode, Data, ColorType, AlphaType};
use skia_safe::effects::gradient_shader::{linear, radial};
use skia_safe::shader::Shader;
use skia_safe::ClipOp;
use skia_safe::paint::{Join, Cap};
use skia_safe::path_effect::PathEffect;

mod text;
mod simple_text;

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

pub struct SkiaImage(skia_safe::Image);

impl Image for SkiaImage {
    fn size(&self) -> Size {
        Size::new(self.0.width().into(), self.0.height().into())
    }
}

fn create_path(shape: impl Shape) -> Path {
    let mut path = Path::new();
    
    for el in shape.path_elements(1e-3) {
        match el {
            PathEl::MoveTo(p) => {
                path.move_to(pairf32(p));
            }
            PathEl::LineTo(p) => {
                path.line_to(pairf32(p));
            }
            PathEl::QuadTo(p1, p2) => {
                path.quad_to(pairf32(p1), pairf32(p2));
            }
            PathEl::CurveTo(p1, p2, p3) => {
                path.cubic_to(pairf32(p1), pairf32(p2), pairf32(p3));
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

#[derive(Debug)]
pub enum SkiaImageError {
    FailedToCreate
}

impl fmt::Display for SkiaImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for SkiaImageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
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
        let gradient = gradient.into();
        let colors_from_stops = |stops: Vec<piet::GradientStop>| {
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
                let mut center = convert_point(center);
                center.x += origin_offset.x as f32;
                center.y += origin_offset.y as f32;
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
        let path = create_path(shape);
        self.canvas.draw_path(&path, &paint);
    }

    fn fill_even_odd(&mut self, _shape: impl Shape, _brush: &impl IntoBrush<Self>) {
        unimplemented!();
    }

    fn clip(&mut self, shape: impl Shape) {
        let path = create_path(shape);
        self.canvas.clip_path(&path, ClipOp::Intersect, false);
    }

    fn stroke(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>, width: f64) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        let mut paint = create_paint();
        apply_brush(&mut paint, brush.as_ref());
        paint.set_stroke_width(width as f32);
        paint.set_style(PaintStyle::Stroke);
        let path = create_path(shape);
        self.canvas.draw_path(&path, &paint);
    }

    fn stroke_styled(
        &mut self,
        shape: impl Shape,
        brush: &impl IntoBrush<Self>,
        width: f64,
        style: &StrokeStyle,
    ) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        let mut paint = create_paint();
        apply_brush(&mut paint, brush.as_ref());
        let line_join = match style.line_join {
            Some(LineJoin::Miter) => {
               Join::Miter 
            }
            Some(LineJoin::Round) => {
                Join::Round
            }
            Some(LineJoin::Bevel) => {
                Join::Bevel
            }
            None => {
                Join::Miter
            }
        };
        let line_cap = match style.line_cap {
            Some(LineCap::Butt) => {
                Cap::Butt
            }
            Some(LineCap::Round) => {
                Cap::Round
            }
            Some(LineCap::Square) => {
                Cap::Square
            }
            None => {
                Cap::Butt
            }
        };
        let path = create_path(shape);
        let path_effect = PathEffect::stroke(width as f32, line_join, line_cap, None);
        paint.set_path_effect(path_effect);
        self.canvas.draw_path(&path, &paint);
    }

    fn text(&mut self) -> &mut Self::Text {
        unimplemented!();
    }

    fn draw_text(&mut self, layout: &Self::TextLayout, pos: impl Into<Point>) {
        let pos = pos.into();
        let rect = layout.image_bounds() + pos.to_vec2();
        let brush = match layout {
            SkiaTextLayout::Paragraph(paragraph) => {
                paragraph.fg_color.make_brush(self, || rect)
            }
            SkiaTextLayout::Simple(simple) => {
                simple.fg_color.make_brush(self, || rect)
            }
        };
        let mut paint = create_paint();
        apply_brush(&mut paint, brush.as_ref());
        
        let mut pos = skia_safe::Point::new(pos.x as f32, pos.y as f32);
        match layout {
            SkiaTextLayout::Paragraph(paragraph) => {
                paragraph.paragraph.paint(&mut self.canvas, pos); 
            }
            SkiaTextLayout::Simple(simple) => {
                for line in simple.line_metrics.iter() {
                    pos.y += line.bounds.height();
                    let text = &simple.text()[line.start_offset..line.end_offset];
                    self.canvas.draw_str(text, pos, &simple.font, &paint);
                }
            }
        };
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
        let dimensions = skia_safe::ISize {
            width: width as i32,
            height: height as i32
        };
        let (color_type, alpha_type) = match format {
            ImageFormat::Rgb => {
                (ColorType::RGB888x, AlphaType::Opaque)
            }
            ImageFormat::RgbaPremul => {
                (ColorType::RGBA8888, AlphaType::Premul)
            }
            ImageFormat::RgbaSeparate => {
                (ColorType::RGBA8888, AlphaType::Unpremul)
            }
            ImageFormat::Grayscale => {
                (ColorType::Gray8, AlphaType::Opaque)
            }
            _ => {
                (ColorType::RGBA8888, AlphaType::Unpremul) 
            }
        };
        let color_space = Some(skia_safe::ColorSpace::new_srgb());
        let row_bytes = width * color_type.bytes_per_pixel();
        let image_info = skia_safe::ImageInfo::new_n32(dimensions, alpha_type, color_space);
        let data = Data::new_copy(buf);
        let image = skia_safe::Image::from_raster_data(&image_info, data, row_bytes).ok_or(
            Error::BackendError(
                Box::new(SkiaImageError::FailedToCreate)
            )
        )?;
        Ok(SkiaImage(image))
    }

    #[inline]
    fn draw_image(
        &mut self,
        image: &Self::Image,
        dst_rect: impl Into<Rect>,
        _interp: InterpolationMode,
    ) {
        let rect = dst_rect.into();
        let left_top = skia_safe::Point::new(rect.x0 as f32, rect.y0 as f32);
        // TODO use interp here
        self.canvas.draw_image(&image.0, left_top, None);
    }

    #[inline]
    fn draw_image_area(
        &mut self,
        _image: &Self::Image,
        _src_rect: impl Into<Rect>,
        _dst_rect: impl Into<Rect>,
        _interp: InterpolationMode,
    ) {
        unimplemented!();
    }

    fn blurred_rect(&mut self, _rect: Rect, _blur_radius: f64, _brush: &impl IntoBrush<Self>) {
        unimplemented!();
    }
}
