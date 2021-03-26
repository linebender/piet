//! The CoreGraphics backend for the Piet 2D graphics abstraction.

#![deny(clippy::trivially_copy_pass_by_ref)]

mod ct_helpers;
mod gradient;
mod text;

use std::borrow::Cow;
use std::sync::Arc;

use core_graphics::base::{
    kCGImageAlphaLast, kCGImageAlphaPremultipliedLast, kCGRenderingIntentDefault, CGFloat,
};
use core_graphics::color_space::CGColorSpace;
use core_graphics::context::{CGContextRef, CGInterpolationQuality, CGLineCap, CGLineJoin};
use core_graphics::data_provider::CGDataProvider;
use core_graphics::geometry::{CGAffineTransform, CGPoint, CGRect, CGSize};
use core_graphics::gradient::CGGradientDrawingOptions;
use core_graphics::image::CGImage;
use foreign_types::ForeignTypeRef;

use piet::kurbo::{Affine, PathEl, Point, QuadBez, Rect, Shape, Size};

use piet::util::unpremul;
use piet::{
    Color, Error, FixedGradient, Image, ImageFormat, InterpolationMode, IntoBrush, LineCap,
    LineJoin, RenderContext, RoundInto, StrokeStyle,
};

pub use crate::text::{CoreGraphicsText, CoreGraphicsTextLayout, CoreGraphicsTextLayoutBuilder};

use gradient::Gradient;

// getting this to be a const takes some gymnastics
const GRADIENT_DRAW_BEFORE_AND_AFTER: CGGradientDrawingOptions =
    CGGradientDrawingOptions::from_bits_truncate(
        CGGradientDrawingOptions::CGGradientDrawsAfterEndLocation.bits()
            | CGGradientDrawingOptions::CGGradientDrawsBeforeStartLocation.bits(),
    );

pub struct CoreGraphicsContext<'a> {
    // Cairo has this as Clone and with &self methods, but we do this to avoid
    // concurrency problems.
    ctx: &'a mut CGContextRef,
    text: CoreGraphicsText,
    // because of the relationship between cocoa and coregraphics (where cocoa
    // may be asked to flip the y-axis) we cannot trust the transform returned
    // by CTContextGetCTM. Instead we maintain our own stack, which will contain
    // only those transforms applied by us.
    transform_stack: Vec<Affine>,
}

impl<'a> CoreGraphicsContext<'a> {
    /// Create a new context with the y-origin at the top-left corner.
    ///
    /// This is not the default for CoreGraphics; but it is the defualt for piet.
    /// To map between the two coordinate spaces you must also pass an explicit
    /// height argument.
    ///
    /// The optional `text` argument can be a reuseable `CoreGraphicsText` struct;
    /// a new one will be constructed if `None` is passed.
    pub fn new_y_up(
        ctx: &mut CGContextRef,
        height: f64,
        text: Option<CoreGraphicsText>,
    ) -> CoreGraphicsContext {
        Self::new_impl(ctx, Some(height), text)
    }

    /// Create a new context with the y-origin at the bottom right corner.
    ///
    /// This is the default for core graphics, but not for piet.
    ///
    /// The optional `text` argument can be a reuseable `CoreGraphicsText` struct;
    /// a new one will be constructed if `None` is passed.
    pub fn new_y_down(
        ctx: &mut CGContextRef,
        text: Option<CoreGraphicsText>,
    ) -> CoreGraphicsContext {
        Self::new_impl(ctx, None, text)
    }

    fn new_impl(
        ctx: &mut CGContextRef,
        height: Option<f64>,
        text: Option<CoreGraphicsText>,
    ) -> CoreGraphicsContext {
        ctx.save();
        if let Some(height) = height {
            let xform = Affine::FLIP_Y * Affine::translate((0.0, -height));
            ctx.concat_ctm(to_cgaffine(xform));
        }
        let text = text.unwrap_or_else(CoreGraphicsText::new_with_unique_state);

        CoreGraphicsContext {
            ctx,
            text,
            transform_stack: Vec::new(),
        }
    }
}

impl<'a> Drop for CoreGraphicsContext<'a> {
    fn drop(&mut self) {
        self.ctx.restore();
    }
}

#[derive(Clone)]
pub enum Brush {
    Solid(Color),
    Gradient(Gradient),
}

/// A core-graphics image
pub enum CoreGraphicsImage {
    /// Empty images are not supported for core-graphics, so we need a variant here to handle that
    /// case.
    Empty,
    NonEmpty(CGImage),
}

impl CoreGraphicsImage {
    fn as_ref(&self) -> Option<&CGImage> {
        match self {
            CoreGraphicsImage::Empty => None,
            CoreGraphicsImage::NonEmpty(img) => Some(img),
        }
    }
}

impl<'a> RenderContext for CoreGraphicsContext<'a> {
    type Brush = Brush;
    type Text = CoreGraphicsText;
    type TextLayout = CoreGraphicsTextLayout;
    type Image = CoreGraphicsImage;

    fn clear(&mut self, region: impl Into<Option<Rect>>, color: Color) {
        // save cannot fail
        let _ = self.save();
        // remove any existing clip
        unsafe {
            CGContextResetClip(self.ctx.as_ptr());
        }
        // remove the current transform
        let current_xform = self.current_transform();
        let xform = current_xform.inverse();
        self.transform(xform);

        let region = region
            .into()
            .map(to_cgrect)
            .unwrap_or_else(|| self.ctx.clip_bounding_box());
        let (r, g, b, a) = color.as_rgba();
        self.ctx
            .set_blend_mode(core_graphics::context::CGBlendMode::Copy);
        self.ctx.set_rgb_fill_color(r, g, b, a);
        self.ctx.fill_rect(region);
        // restore cannot fail, because we saved at the start of the method
        self.restore().unwrap();
    }

    fn solid_brush(&mut self, color: Color) -> Brush {
        Brush::Solid(color)
    }

    fn gradient(&mut self, gradient: impl Into<FixedGradient>) -> Result<Brush, Error> {
        let gradient = Gradient::from_piet_gradient(gradient.into());
        Ok(Brush::Gradient(gradient))
    }

    /// Fill a shape.
    fn fill(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        self.set_path(shape);
        match brush.as_ref() {
            Brush::Solid(color) => {
                self.set_fill_color(color);
                self.ctx.fill_path();
            }
            Brush::Gradient(grad) => {
                self.ctx.save();
                self.ctx.clip();
                grad.fill(self.ctx, GRADIENT_DRAW_BEFORE_AND_AFTER);
                self.ctx.restore();
            }
        }
    }

    fn fill_even_odd(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        self.set_path(shape);
        match brush.as_ref() {
            Brush::Solid(color) => {
                self.set_fill_color(color);
                self.ctx.fill_path();
            }
            Brush::Gradient(grad) => {
                self.ctx.save();
                self.ctx.eo_clip();
                grad.fill(self.ctx, GRADIENT_DRAW_BEFORE_AND_AFTER);
                self.ctx.restore();
            }
        }
    }

    fn clip(&mut self, shape: impl Shape) {
        self.set_path(shape);
        self.ctx.clip();
    }

    fn stroke(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>, width: f64) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        self.set_path(shape);
        self.set_stroke(width.round_into(), None);
        match brush.as_ref() {
            Brush::Solid(color) => {
                self.set_stroke_color(color);
                self.ctx.stroke_path();
            }
            Brush::Gradient(grad) => {
                self.ctx.save();
                self.ctx.replace_path_with_stroked_path();
                self.ctx.clip();
                grad.fill(self.ctx, GRADIENT_DRAW_BEFORE_AND_AFTER);
                self.ctx.restore();
            }
        }
    }

    fn stroke_styled(
        &mut self,
        shape: impl Shape,
        brush: &impl IntoBrush<Self>,
        width: f64,
        style: &StrokeStyle,
    ) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        self.set_path(shape);
        self.set_stroke(width.round_into(), Some(style));
        match brush.as_ref() {
            Brush::Solid(color) => {
                self.set_stroke_color(color);
                self.ctx.stroke_path();
            }
            Brush::Gradient(grad) => {
                self.ctx.save();
                self.ctx.replace_path_with_stroked_path();
                self.ctx.clip();
                grad.fill(self.ctx, GRADIENT_DRAW_BEFORE_AND_AFTER);
                self.ctx.restore();
            }
        }
    }

    fn text(&mut self) -> &mut Self::Text {
        &mut self.text
    }

    fn draw_text(&mut self, layout: &Self::TextLayout, pos: impl Into<Point>) {
        let pos = pos.into();
        self.ctx.save();
        // inverted coordinate system; text is drawn from bottom left corner,
        // and (0, 0) in context is also bottom left.
        self.ctx.translate(pos.x, layout.frame_size.height + pos.y);
        self.ctx.scale(1.0, -1.0);
        layout.draw(self.ctx);
        self.ctx.restore();
    }

    fn save(&mut self) -> Result<(), Error> {
        self.ctx.save();
        let state = self.transform_stack.last().copied().unwrap_or_default();
        self.transform_stack.push(state);
        Ok(())
    }

    fn restore(&mut self) -> Result<(), Error> {
        if self.transform_stack.pop().is_some() {
            // we're defensive about calling restore on the inner context,
            // because an unbalanced call will trigger an assert in C
            self.ctx.restore();
            Ok(())
        } else {
            Err(Error::StackUnbalance)
        }
    }

    fn finish(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn transform(&mut self, transform: Affine) {
        if let Some(last) = self.transform_stack.last_mut() {
            *last *= transform;
        } else {
            self.transform_stack.push(transform);
        }
        self.ctx.concat_ctm(to_cgaffine(transform));
    }

    fn make_image(
        &mut self,
        width: usize,
        height: usize,
        buf: &[u8],
        format: ImageFormat,
    ) -> Result<Self::Image, Error> {
        if width == 0 || height == 0 {
            return Ok(CoreGraphicsImage::Empty);
        }
        assert!(!buf.is_empty() && buf.len() <= format.bytes_per_pixel() * width * height);
        let data = Arc::new(buf.to_owned());
        let data_provider = CGDataProvider::from_buffer(data);
        let (colorspace, bitmap_info, bytes) = match format {
            ImageFormat::Rgb => (CGColorSpace::create_device_rgb(), 0, 3),
            ImageFormat::RgbaPremul => (
                CGColorSpace::create_device_rgb(),
                kCGImageAlphaPremultipliedLast,
                4,
            ),
            ImageFormat::RgbaSeparate => (CGColorSpace::create_device_rgb(), kCGImageAlphaLast, 4),
            ImageFormat::Grayscale => (CGColorSpace::create_device_gray(), 0, 1),
            _ => unimplemented!(),
        };
        let bits_per_component = 8;
        // this doesn't matter, we set interpolation mode manually in draw_image
        let should_interpolate = false;
        let rendering_intent = kCGRenderingIntentDefault;
        let image = CGImage::new(
            width,
            height,
            bits_per_component,
            bytes * bits_per_component,
            width * bytes,
            &colorspace,
            bitmap_info,
            &data_provider,
            should_interpolate,
            rendering_intent,
        );
        Ok(CoreGraphicsImage::NonEmpty(image))
    }

    fn draw_image(
        &mut self,
        image: &Self::Image,
        rect: impl Into<Rect>,
        interp: InterpolationMode,
    ) {
        let image = match image.as_ref() {
            Some(img) => img,
            None => return,
        };
        self.ctx.save();
        //https://developer.apple.com/documentation/coregraphics/cginterpolationquality?language=objc
        let quality = match interp {
            InterpolationMode::NearestNeighbor => {
                CGInterpolationQuality::CGInterpolationQualityNone
            }
            InterpolationMode::Bilinear => CGInterpolationQuality::CGInterpolationQualityDefault,
        };
        self.ctx.set_interpolation_quality(quality);
        let rect = rect.into();
        // CGImage is drawn flipped by default; it's easier for us to handle
        // this transformation if we're drawing into a rect at the origin, so
        // we convert our origin into a translation of the drawing context.
        self.ctx.translate(rect.min_x(), rect.max_y());
        self.ctx.scale(1.0, -1.0);
        self.ctx
            .draw_image(to_cgrect(rect.with_origin(Point::ZERO)), image);
        self.ctx.restore();
    }

    fn draw_image_area(
        &mut self,
        image: &Self::Image,
        src_rect: impl Into<Rect>,
        dst_rect: impl Into<Rect>,
        _interp: InterpolationMode,
    ) {
        if let CoreGraphicsImage::NonEmpty(image) = image {
            if let Some(cropped) = image.cropped(to_cgrect(src_rect)) {
                // TODO: apply interpolation mode
                self.ctx.draw_image(to_cgrect(dst_rect), &cropped);
            }
        }
    }

    fn capture_image_area(&mut self, src_rect: impl Into<Rect>) -> Result<Self::Image, Error> {
        let src_rect = src_rect.into();
        if src_rect.width() < 1.0 || src_rect.height() < 1.0 {
            return Err(Error::InvalidInput);
        }

        if src_rect.width() > self.ctx.width() as f64
            || src_rect.height() > self.ctx.height() as f64
        {
            return Err(Error::InvalidInput);
        }

        let full_image = self.ctx.create_image().ok_or(Error::InvalidInput)?;

        if src_rect.width().round() as usize == self.ctx.width()
            && src_rect.height().round() as usize == self.ctx.height()
        {
            return Ok(CoreGraphicsImage::NonEmpty(full_image));
        }

        full_image
            .cropped(to_cgrect(src_rect))
            .map(CoreGraphicsImage::NonEmpty)
            .ok_or(Error::InvalidInput)
    }

    fn blurred_rect(&mut self, rect: Rect, blur_radius: f64, brush: &impl IntoBrush<Self>) {
        let (image, rect) = compute_blurred_rect(rect, blur_radius);
        let cg_rect = to_cgrect(rect);
        self.ctx.save();
        self.ctx.clip_to_mask(cg_rect, &image);
        self.fill(rect, brush);
        self.ctx.restore()
    }

    fn current_transform(&self) -> Affine {
        self.transform_stack.last().copied().unwrap_or_default()
    }

    fn status(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

impl<'a> IntoBrush<CoreGraphicsContext<'a>> for Brush {
    fn make_brush<'b>(
        &'b self,
        _piet: &mut CoreGraphicsContext,
        _bbox: impl FnOnce() -> Rect,
    ) -> std::borrow::Cow<'b, Brush> {
        Cow::Borrowed(self)
    }
}

impl Image for CoreGraphicsImage {
    fn size(&self) -> Size {
        // `size_t` (which could be 64 bits wide) does not losslessly convert to `f64`. In
        // reality, the image you're working with would have to be pretty big to be an issue, and
        // the issue would only be accuracy of the size.
        match self {
            CoreGraphicsImage::Empty => Size::new(0., 0.),
            CoreGraphicsImage::NonEmpty(image) => {
                Size::new(image.width() as f64, image.height() as f64)
            }
        }
    }
}

fn convert_line_join(line_join: LineJoin) -> CGLineJoin {
    match line_join {
        LineJoin::Miter { .. } => CGLineJoin::CGLineJoinMiter,
        LineJoin::Round => CGLineJoin::CGLineJoinRound,
        LineJoin::Bevel => CGLineJoin::CGLineJoinBevel,
    }
}

fn convert_line_cap(line_cap: LineCap) -> CGLineCap {
    #[allow(deprecated)]
    match line_cap {
        LineCap::Butt => CGLineCap::CGLineCapButt,
        LineCap::Round => CGLineCap::CGLineCapRound,
        LineCap::Square => CGLineCap::CGLineCapSquare,
    }
}

impl<'a> CoreGraphicsContext<'a> {
    fn set_fill_color(&mut self, color: &Color) {
        let (r, g, b, a) = Color::as_rgba(&color);
        self.ctx.set_rgb_fill_color(r, g, b, a);
    }

    fn set_stroke_color(&mut self, color: &Color) {
        let (r, g, b, a) = Color::as_rgba(&color);
        self.ctx.set_rgb_stroke_color(r, g, b, a);
    }

    /// Set the stroke parameters.
    fn set_stroke(&mut self, width: f64, style: Option<&StrokeStyle>) {
        let default_style = StrokeStyle::default();
        let style = style.unwrap_or(&default_style);
        self.ctx.set_line_width(width);

        self.ctx.set_line_join(convert_line_join(style.line_join));
        self.ctx.set_line_cap(convert_line_cap(style.line_cap));

        if let Some(limit) = style.miter_limit() {
            self.ctx.set_miter_limit(limit);
        }

        self.ctx
            .set_line_dash(style.dash_offset, &style.dash_pattern);
    }

    fn set_path(&mut self, shape: impl Shape) {
        // This shouldn't be necessary, we always leave the context in no-path
        // state. But just in case, and it should be harmless.
        self.ctx.begin_path();
        let mut last = Point::default();
        for el in shape.path_elements(1e-3) {
            match el {
                PathEl::MoveTo(p) => {
                    self.ctx.move_to_point(p.x, p.y);
                    last = p;
                }
                PathEl::LineTo(p) => {
                    self.ctx.add_line_to_point(p.x, p.y);
                    last = p;
                }
                PathEl::QuadTo(p1, p2) => {
                    let q = QuadBez::new(last, p1, p2);
                    let c = q.raise();
                    self.ctx
                        .add_curve_to_point(c.p1.x, c.p1.y, c.p2.x, c.p2.y, p2.x, p2.y);
                    last = p2;
                }
                PathEl::CurveTo(p1, p2, p3) => {
                    self.ctx
                        .add_curve_to_point(p1.x, p1.y, p2.x, p2.y, p3.x, p3.y);
                    last = p3;
                }
                PathEl::ClosePath => self.ctx.close_path(),
            }
        }
    }
}

fn compute_blurred_rect(rect: Rect, radius: f64) -> (CGImage, Rect) {
    let size = piet::util::size_for_blurred_rect(rect, radius);
    let width = size.width as usize;
    let height = size.height as usize;

    let mut data = vec![0u8; width * height];
    let rect_exp = piet::util::compute_blurred_rect(rect, radius, width, &mut data);

    let data_provider = CGDataProvider::from_buffer(Arc::new(data));
    let color_space = CGColorSpace::create_device_gray();
    let image = CGImage::new(
        width,
        height,
        8,
        8,
        width,
        &color_space,
        0,
        &data_provider,
        false,
        0,
    );
    (image, rect_exp)
}

fn to_cgpoint(point: Point) -> CGPoint {
    CGPoint::new(point.x as CGFloat, point.y as CGFloat)
}

fn to_cgsize(size: Size) -> CGSize {
    CGSize::new(size.width, size.height)
}

fn to_cgrect(rect: impl Into<Rect>) -> CGRect {
    let rect = rect.into();
    CGRect::new(&to_cgpoint(rect.origin()), &to_cgsize(rect.size()))
}

fn to_cgaffine(affine: Affine) -> CGAffineTransform {
    let [a, b, c, d, tx, ty] = affine.as_coeffs();
    CGAffineTransform::new(a, b, c, d, tx, ty)
}

#[allow(dead_code)]
pub fn unpremultiply_rgba(data: &mut [u8]) {
    for i in (0..data.len()).step_by(4) {
        let a = data[i + 3];
        if a != 0 {
            for x in &mut data[i..(i + 3)] {
                *x = unpremul(*x, a);
            }
        }
    }
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGContextResetClip(c: core_graphics::sys::CGContextRef);
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_graphics::color_space::CGColorSpace;
    use core_graphics::context::CGContext;

    fn make_context(size: impl Into<Size>) -> CGContext {
        let size = size.into();
        CGContext::create_bitmap_context(
            None,
            size.width as usize,
            size.height as usize,
            8,
            0,
            &CGColorSpace::create_device_rgb(),
            core_graphics::base::kCGImageAlphaPremultipliedLast,
        )
    }

    fn equalish_affine(one: Affine, two: Affine) -> bool {
        one.as_coeffs()
            .iter()
            .zip(two.as_coeffs().iter())
            .all(|(a, b)| (a - b).abs() < f64::EPSILON)
    }

    macro_rules! assert_affine_eq {
        ($left:expr, $right:expr) => {{
            if !equalish_affine($left, $right) {
                panic!(
                    "assertion failed: `(one == two)`\n\
                one: {:?}\n\
                two: {:?}",
                    $left.as_coeffs(),
                    $right.as_coeffs()
                )
            }
        }};
    }

    #[test]
    fn get_affine_y_up() {
        let mut ctx = make_context((400.0, 400.0));
        let mut piet = CoreGraphicsContext::new_y_up(&mut ctx, 400.0, None);
        let affine = piet.current_transform();
        assert_affine_eq!(affine, Affine::default());

        let one = Affine::translate((50.0, 20.0));
        let two = Affine::rotate(2.2);
        let three = Affine::FLIP_Y;
        let four = Affine::scale_non_uniform(2.0, -1.5);

        piet.save().unwrap();
        piet.transform(one);
        piet.transform(one);
        piet.save().unwrap();
        piet.transform(two);
        piet.save().unwrap();
        piet.transform(three);
        assert_affine_eq!(piet.current_transform(), one * one * two * three);
        piet.transform(four);
        piet.save().unwrap();

        assert_affine_eq!(piet.current_transform(), one * one * two * three * four);
        piet.restore().unwrap();
        assert_affine_eq!(piet.current_transform(), one * one * two * three * four);
        piet.restore().unwrap();
        assert_affine_eq!(piet.current_transform(), one * one * two);
        piet.restore().unwrap();
        assert_affine_eq!(piet.current_transform(), one * one);
        piet.restore().unwrap();
        assert_affine_eq!(piet.current_transform(), Affine::default());
    }

    #[test]
    fn capture_image_area() {
        let mut ctx = make_context((400.0, 400.0));
        let mut piet = CoreGraphicsContext::new_y_down(&mut ctx, None);

        assert!(piet
            .capture_image_area(Rect::new(0.0, 0.0, 0.0, 0.0))
            .is_err());
        assert!(piet
            .capture_image_area(Rect::new(0.0, 0.0, 500.0, 400.0))
            .is_err());
        assert!(piet
            .capture_image_area(Rect::new(100.0, 100.0, 200.0, 200.0))
            .is_ok());

        let copy = piet
            .capture_image_area(Rect::new(100.0, 100.0, 200.0, 200.0))
            .unwrap();
        piet.draw_image(
            &copy,
            Rect::new(0.0, 0.0, 400.0, 400.0),
            InterpolationMode::Bilinear,
        );
    }
}
