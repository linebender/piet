//! The CoreGraphics backend for the Piet 2D graphics abstraction.

mod text;

use std::borrow::Cow;
use std::sync::Arc;

use core_graphics::base::{
    kCGImageAlphaLast, kCGImageAlphaPremultipliedLast, kCGRenderingIntentDefault, CGFloat,
};
use core_graphics::color_space::CGColorSpace;
use core_graphics::context::{CGContext, CGLineCap, CGLineJoin};
use core_graphics::data_provider::CGDataProvider;
use core_graphics::geometry::{CGAffineTransform, CGPoint, CGRect, CGSize};
use core_graphics::image::CGImage;

use piet::kurbo::{Affine, PathEl, Point, QuadBez, Rect, Shape, Size};

use piet::{
    Color, Error, FixedGradient, ImageFormat, InterpolationMode, IntoBrush, LineCap, LineJoin,
    RenderContext, RoundInto, StrokeStyle,
};

pub use crate::text::{
    CoreGraphicsFont, CoreGraphicsFontBuilder, CoreGraphicsText, CoreGraphicsTextLayout,
    CoreGraphicsTextLayoutBuilder,
};

pub struct CoreGraphicsContext<'a> {
    // Cairo has this as Clone and with &self methods, but we do this to avoid
    // concurrency problems.
    ctx: &'a mut CGContext,
    text: CoreGraphicsText,
}

impl<'a> CoreGraphicsContext<'a> {
    pub fn new(ctx: &mut CGContext) -> CoreGraphicsContext {
        CoreGraphicsContext {
            ctx,
            text: CoreGraphicsText,
        }
    }
}

#[derive(Clone)]
pub enum Brush {
    Solid(u32),
    Gradient,
}

impl<'a> RenderContext for CoreGraphicsContext<'a> {
    type Brush = Brush;
    type Text = CoreGraphicsText;
    type TextLayout = CoreGraphicsTextLayout;
    type Image = CGImage;
    //type StrokeStyle = StrokeStyle;

    fn clear(&mut self, color: Color) {
        let rgba = color.as_rgba_u32();
        self.ctx.set_rgb_fill_color(
            byte_to_frac(rgba >> 24),
            byte_to_frac(rgba >> 16),
            byte_to_frac(rgba >> 8),
            byte_to_frac(rgba),
        );
        self.ctx.fill_rect(self.ctx.clip_bounding_box());
    }

    fn solid_brush(&mut self, color: Color) -> Brush {
        Brush::Solid(color.as_rgba_u32())
    }

    fn gradient(&mut self, _gradient: impl Into<FixedGradient>) -> Result<Brush, Error> {
        unimplemented!()
    }

    /// Fill a shape.
    fn fill(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        self.set_path(shape);
        self.set_fill_brush(&brush);
        self.ctx.fill_path();
    }

    fn fill_even_odd(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        self.set_path(shape);
        self.set_fill_brush(&brush);
        self.ctx.eo_fill_path();
    }

    fn clip(&mut self, shape: impl Shape) {
        self.set_path(shape);
        self.ctx.clip();
    }

    fn stroke(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>, width: f64) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        self.set_path(shape);
        self.set_stroke(width.round_into(), None);
        self.set_stroke_brush(&brush);
        self.ctx.stroke_path();
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
        self.set_stroke_brush(&brush);
        self.ctx.stroke_path();
    }

    fn text(&mut self) -> &mut Self::Text {
        &mut self.text
    }

    fn draw_text(
        &mut self,
        _layout: &Self::TextLayout,
        _pos: impl Into<Point>,
        _brush: &impl IntoBrush<Self>,
    ) {
        unimplemented!()
    }

    fn save(&mut self) -> Result<(), Error> {
        self.ctx.save();
        Ok(())
    }

    fn restore(&mut self) -> Result<(), Error> {
        self.ctx.restore();
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn transform(&mut self, transform: Affine) {
        let transform = to_cgaffine(transform);
        self.ctx.concat_ctm(transform);
    }

    fn make_image(
        &mut self,
        width: usize,
        height: usize,
        buf: &[u8],
        format: ImageFormat,
    ) -> Result<Self::Image, Error> {
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
            _ => unimplemented!(),
        };
        let bits_per_component = 8;
        // TODO: we don't know this until drawing time, so defer actual image creation til then.
        let should_interpolate = true;
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
        Ok(image)
    }

    fn draw_image(
        &mut self,
        image: &Self::Image,
        rect: impl Into<Rect>,
        _interp: InterpolationMode,
    ) {
        // TODO: apply interpolation mode
        self.ctx.draw_image(to_cgrect(rect), image);
    }

    fn draw_image_area(
        &mut self,
        image: &Self::Image,
        src_rect: impl Into<Rect>,
        dst_rect: impl Into<Rect>,
        _interp: InterpolationMode,
    ) {
        if let Some(cropped) = image.cropped(to_cgrect(src_rect)) {
            // TODO: apply interpolation mode
            self.ctx.draw_image(to_cgrect(dst_rect), &cropped);
        }
    }

    fn blurred_rect(&mut self, _rect: Rect, _blur_radius: f64, _brush: &impl IntoBrush<Self>) {
        unimplemented!()
    }

    fn current_transform(&self) -> Affine {
        let ctm = self.ctx.get_ctm();
        from_cgaffine(ctm)
    }

    fn status(&mut self) -> Result<(), Error> {
        unimplemented!()
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

fn convert_line_join(line_join: LineJoin) -> CGLineJoin {
    match line_join {
        LineJoin::Miter => CGLineJoin::CGLineJoinMiter,
        LineJoin::Round => CGLineJoin::CGLineJoinRound,
        LineJoin::Bevel => CGLineJoin::CGLineJoinBevel,
    }
}

fn convert_line_cap(line_cap: LineCap) -> CGLineCap {
    match line_cap {
        LineCap::Butt => CGLineCap::CGLineCapButt,
        LineCap::Round => CGLineCap::CGLineCapRound,
        LineCap::Square => CGLineCap::CGLineCapSquare,
    }
}

impl<'a> CoreGraphicsContext<'a> {
    /// Set the source pattern to the brush.
    ///
    /// Cairo is super stateful, and we're trying to have more retained stuff.
    /// This is part of the impedance matching.
    fn set_fill_brush(&mut self, brush: &Brush) {
        match *brush {
            Brush::Solid(rgba) => self.ctx.set_rgb_fill_color(
                byte_to_frac(rgba >> 24),
                byte_to_frac(rgba >> 16),
                byte_to_frac(rgba >> 8),
                byte_to_frac(rgba),
            ),
            Brush::Gradient => unimplemented!(),
        }
    }

    fn set_stroke_brush(&mut self, brush: &Brush) {
        match *brush {
            Brush::Solid(rgba) => self.ctx.set_rgb_stroke_color(
                byte_to_frac(rgba >> 24),
                byte_to_frac(rgba >> 16),
                byte_to_frac(rgba >> 8),
                byte_to_frac(rgba),
            ),
            Brush::Gradient => unimplemented!(),
        }
    }

    /// Set the stroke parameters.
    fn set_stroke(&mut self, width: f64, style: Option<&StrokeStyle>) {
        self.ctx.set_line_width(width);

        let line_join = style
            .and_then(|style| style.line_join)
            .unwrap_or(LineJoin::Miter);
        self.ctx.set_line_join(convert_line_join(line_join));

        let line_cap = style
            .and_then(|style| style.line_cap)
            .unwrap_or(LineCap::Butt);
        self.ctx.set_line_cap(convert_line_cap(line_cap));

        let miter_limit = style.and_then(|style| style.miter_limit).unwrap_or(10.0);
        self.ctx.set_miter_limit(miter_limit);

        match style.and_then(|style| style.dash.as_ref()) {
            None => self.ctx.set_line_dash(0.0, &[]),
            Some((dashes, offset)) => self.ctx.set_line_dash(*offset, dashes),
        }
    }

    fn set_path(&mut self, shape: impl Shape) {
        // This shouldn't be necessary, we always leave the context in no-path
        // state. But just in case, and it should be harmless.
        self.ctx.begin_path();
        let mut last = Point::default();
        for el in shape.to_bez_path(1e-3) {
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

fn byte_to_frac(byte: u32) -> f64 {
    ((byte & 255) as f64) * (1.0 / 255.0)
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

fn from_cgaffine(affine: CGAffineTransform) -> Affine {
    let CGAffineTransform { a, b, c, d, tx, ty } = affine;
    Affine::new([a, b, c, d, tx, ty])
}

fn to_cgaffine(affine: Affine) -> CGAffineTransform {
    let [a, b, c, d, tx, ty] = affine.as_coeffs();
    CGAffineTransform::new(a, b, c, d, tx, ty)
}
