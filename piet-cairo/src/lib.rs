//! The Cairo backend for the Piet 2D graphics abstraction.

#![deny(clippy::trivially_copy_pass_by_ref)]

mod text;

use std::borrow::Cow;

use cairo::{Context, Filter, Format, ImageSurface, Matrix, SurfacePattern};

use piet::kurbo::{Affine, PathEl, Point, QuadBez, Rect, Shape, Size};
use piet::{
    Color, Error, FixedGradient, Image, ImageFormat, InterpolationMode, IntoBrush, LineCap,
    LineJoin, RenderContext, StrokeStyle,
};

pub use crate::text::{CairoText, CairoTextLayout, CairoTextLayoutBuilder};

pub struct CairoRenderContext<'a> {
    // Cairo has this as Clone and with &self methods, but we do this to avoid
    // concurrency problems.
    ctx: &'a Context,
    text: CairoText,
    // because of the relationship between GTK and cairo (where GTK applies a transform
    // to adjust for menus and window borders) we cannot trust the transform returned
    // by cairo. Instead we maintain our own stack, which will contain
    // only those transforms applied by us.
    transform_stack: Vec<Affine>,
}

impl<'a> CairoRenderContext<'a> {}

#[derive(Clone)]
pub enum Brush {
    Solid(u32),
    Linear(cairo::LinearGradient),
    Radial(cairo::RadialGradient),
}

pub struct CairoImage(ImageSurface);

// we call this with different types of gradient that have `add_color_stop_rgba` fns,
// and there's no trait for this behaviour so we use a macro. ¯\_(ツ)_/¯
macro_rules! set_gradient_stops {
    ($dst: expr, $stops: expr) => {
        for stop in $stops {
            let rgba = stop.color.as_rgba_u32();
            $dst.add_color_stop_rgba(
                stop.pos as f64,
                byte_to_frac(rgba >> 24),
                byte_to_frac(rgba >> 16),
                byte_to_frac(rgba >> 8),
                byte_to_frac(rgba),
            );
        }
    };
}

impl<'a> RenderContext for CairoRenderContext<'a> {
    type Brush = Brush;

    type Text = CairoText;
    type TextLayout = CairoTextLayout;

    type Image = CairoImage;

    fn status(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn clear(&mut self, region: impl Into<Option<Rect>>, color: Color) {
        let region: Option<Rect> = region.into();
        let _ = self.with_save(|rc| {
            rc.ctx.reset_clip();
            // we DO want to clip the specified region and reset the transformation
            if let Some(region) = region {
                rc.transform(rc.current_transform().inverse());
                rc.clip(region);
            }

            //prepare the colors etc
            let rgba = color.as_rgba_u32();
            rc.ctx.set_source_rgba(
                byte_to_frac(rgba >> 24),
                byte_to_frac(rgba >> 16),
                byte_to_frac(rgba >> 8),
                byte_to_frac(rgba),
            );
            rc.ctx.set_operator(cairo::Operator::Source);
            rc.ctx.paint();
            Ok(())
        });
    }

    fn solid_brush(&mut self, color: Color) -> Brush {
        Brush::Solid(color.as_rgba_u32())
    }

    fn gradient(&mut self, gradient: impl Into<FixedGradient>) -> Result<Brush, Error> {
        match gradient.into() {
            FixedGradient::Linear(linear) => {
                let (x0, y0) = (linear.start.x, linear.start.y);
                let (x1, y1) = (linear.end.x, linear.end.y);
                let lg = cairo::LinearGradient::new(x0, y0, x1, y1);
                set_gradient_stops!(&lg, &linear.stops);
                Ok(Brush::Linear(lg))
            }
            FixedGradient::Radial(radial) => {
                let (xc, yc) = (radial.center.x, radial.center.y);
                let (xo, yo) = (radial.origin_offset.x, radial.origin_offset.y);
                let r = radial.radius;
                let rg = cairo::RadialGradient::new(xc + xo, yc + yo, 0.0, xc, yc, r);
                set_gradient_stops!(&rg, &radial.stops);
                Ok(Brush::Radial(rg))
            }
        }
    }

    fn fill(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        self.set_path(shape);
        self.set_brush(&*brush);
        self.ctx.set_fill_rule(cairo::FillRule::Winding);
        self.ctx.fill();
    }

    fn fill_even_odd(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        self.set_path(shape);
        self.set_brush(&*brush);
        self.ctx.set_fill_rule(cairo::FillRule::EvenOdd);
        self.ctx.fill();
    }

    fn clip(&mut self, shape: impl Shape) {
        self.set_path(shape);
        self.ctx.set_fill_rule(cairo::FillRule::Winding);
        self.ctx.clip();
    }

    fn stroke(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>, width: f64) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        self.set_path(shape);
        self.set_stroke(width, None);
        self.set_brush(&*brush);
        self.ctx.stroke();
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
        self.set_stroke(width, Some(style));
        self.set_brush(&*brush);
        self.ctx.stroke();
    }

    fn text(&mut self) -> &mut Self::Text {
        &mut self.text
    }

    fn draw_text(&mut self, layout: &Self::TextLayout, pos: impl Into<Point>) {
        let pos = pos.into();
        let offset = layout.pango_offset();
        self.ctx.move_to(pos.x - offset.x, pos.y - offset.y);
        pangocairo::show_layout(&self.ctx, layout.pango_layout());
    }

    fn save(&mut self) -> Result<(), Error> {
        self.ctx.save();
        let state = self.transform_stack.last().copied().unwrap_or_default();
        self.transform_stack.push(state);
        self.status()
    }

    fn restore(&mut self) -> Result<(), Error> {
        if self.transform_stack.pop().is_some() {
            // we're defensive about calling restore on the inner context,
            // because an unbalanced call will trigger a panic in cairo-rs
            self.ctx.restore();
            self.status()
        } else {
            Err(Error::StackUnbalance)
        }
    }

    fn finish(&mut self) -> Result<(), Error> {
        self.ctx.get_target().flush();
        self.status()
    }

    fn transform(&mut self, transform: Affine) {
        if let Some(last) = self.transform_stack.last_mut() {
            *last *= transform;
        } else {
            self.transform_stack.push(transform);
        }
        self.ctx.transform(affine_to_matrix(transform));
    }

    fn current_transform(&self) -> Affine {
        self.transform_stack.last().copied().unwrap_or_default()
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
        let cairo_fmt = match format {
            ImageFormat::Rgb | ImageFormat::Grayscale => Format::Rgb24,
            ImageFormat::RgbaSeparate | ImageFormat::RgbaPremul => Format::ARgb32,
            _ => return Err(Error::NotSupported),
        };
        let width_int = width as i32;
        let height_int = height as i32;
        let mut image = ImageSurface::create(cairo_fmt, width_int, height_int)
            .map_err(|e| Error::BackendError(Box::new(e)))?;

        // early-return if the image has no data in it
        if width_int == 0 || height_int == 0 {
            return Ok(CairoImage(image));
        }

        // Confident no borrow errors because we just created it.
        let bytes_per_pixel = format.bytes_per_pixel();
        let bytes_per_row = width * bytes_per_pixel;
        let stride = image.get_stride() as usize;
        {
            let mut data = image
                .get_data()
                .map_err(|e| Error::BackendError(Box::new(e)))?;
            for y in 0..height {
                let src_off = y * bytes_per_row;
                let data = &mut data[y * stride..];
                match format {
                    ImageFormat::Rgb => {
                        for x in 0..width {
                            write_rgb(
                                data,
                                x,
                                buf[src_off + x * 3 + 0],
                                buf[src_off + x * 3 + 1],
                                buf[src_off + x * 3 + 2],
                            );
                        }
                    }
                    ImageFormat::RgbaPremul => {
                        // It's annoying that Cairo exposes only ARGB. Ah well. Let's
                        // hope that LLVM generates pretty good code for this.
                        // TODO: consider adding BgraPremul format.
                        for x in 0..width {
                            write_rgba(
                                data,
                                x,
                                buf[src_off + x * 4 + 0],
                                buf[src_off + x * 4 + 1],
                                buf[src_off + x * 4 + 2],
                                buf[src_off + x * 4 + 3],
                            );
                        }
                    }
                    ImageFormat::RgbaSeparate => {
                        fn premul(x: u8, a: u8) -> u8 {
                            let y = (x as u16) * (a as u16);
                            ((y + (y >> 8) + 0x80) >> 8) as u8
                        }
                        for x in 0..width {
                            let a = buf[src_off + x * 4 + 3];
                            write_rgba(
                                data,
                                x,
                                premul(buf[src_off + x * 4 + 0], a),
                                premul(buf[src_off + x * 4 + 1], a),
                                premul(buf[src_off + x * 4 + 2], a),
                                a,
                            );
                        }
                    }
                    ImageFormat::Grayscale => {
                        for x in 0..width {
                            write_rgb(
                                data,
                                x,
                                buf[src_off + x],
                                buf[src_off + x],
                                buf[src_off + x],
                            );
                        }
                    }
                    _ => return Err(Error::NotSupported),
                }
            }
        }
        Ok(CairoImage(image))
    }

    #[inline]
    fn draw_image(
        &mut self,
        image: &Self::Image,
        dst_rect: impl Into<Rect>,
        interp: InterpolationMode,
    ) {
        self.draw_image_inner(&image.0, None, dst_rect.into(), interp);
    }

    #[inline]
    fn draw_image_area(
        &mut self,
        image: &Self::Image,
        src_rect: impl Into<Rect>,
        dst_rect: impl Into<Rect>,
        interp: InterpolationMode,
    ) {
        self.draw_image_inner(&image.0, Some(src_rect.into()), dst_rect.into(), interp);
    }

    fn capture_image_area(&mut self, _src_rect: impl Into<Rect>) -> Result<Self::Image, Error> {
        Err(Error::Unimplemented)
    }

    fn blurred_rect(&mut self, rect: Rect, blur_radius: f64, brush: &impl IntoBrush<Self>) {
        let brush = brush.make_brush(self, || rect);
        let (image, origin) = compute_blurred_rect(rect, blur_radius);
        self.set_brush(&*brush);
        self.ctx.mask_surface(&image, origin.x, origin.y);
    }
}

impl<'a> IntoBrush<CairoRenderContext<'a>> for Brush {
    fn make_brush<'b>(
        &'b self,
        _piet: &mut CairoRenderContext,
        _bbox: impl FnOnce() -> Rect,
    ) -> std::borrow::Cow<'b, Brush> {
        Cow::Borrowed(self)
    }
}

impl Image for CairoImage {
    fn size(&self) -> Size {
        Size::new(self.0.get_width().into(), self.0.get_height().into())
    }
}

impl<'a> CairoRenderContext<'a> {
    /// Create a new Cairo back-end.
    ///
    /// At the moment, it uses the "toy text API" for text layout, but when
    /// we change to a more sophisticated text layout approach, we'll probably
    /// need a factory for that as an additional argument.
    pub fn new(ctx: &Context) -> CairoRenderContext {
        CairoRenderContext {
            ctx,
            text: CairoText::new(),
            transform_stack: Vec::new(),
        }
    }

    /// Set the source pattern to the brush.
    ///
    /// Cairo is super stateful, and we're trying to have more retained stuff.
    /// This is part of the impedance matching.
    fn set_brush(&mut self, brush: &Brush) {
        match *brush {
            Brush::Solid(rgba) => self.ctx.set_source_rgba(
                byte_to_frac(rgba >> 24),
                byte_to_frac(rgba >> 16),
                byte_to_frac(rgba >> 8),
                byte_to_frac(rgba),
            ),
            Brush::Linear(ref linear) => self.ctx.set_source(linear),
            Brush::Radial(ref radial) => self.ctx.set_source(radial),
        }
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
        self.ctx.set_dash(&style.dash_pattern, style.dash_offset);
    }

    fn set_path(&mut self, shape: impl Shape) {
        // This shouldn't be necessary, we always leave the context in no-path
        // state. But just in case, and it should be harmless.
        self.ctx.new_path();
        let mut last = Point::ZERO;
        for el in shape.path_elements(1e-3) {
            match el {
                PathEl::MoveTo(p) => {
                    self.ctx.move_to(p.x, p.y);
                    last = p;
                }
                PathEl::LineTo(p) => {
                    self.ctx.line_to(p.x, p.y);
                    last = p;
                }
                PathEl::QuadTo(p1, p2) => {
                    let q = QuadBez::new(last, p1, p2);
                    let c = q.raise();
                    self.ctx
                        .curve_to(c.p1.x, c.p1.y, c.p2.x, c.p2.y, p2.x, p2.y);
                    last = p2;
                }
                PathEl::CurveTo(p1, p2, p3) => {
                    self.ctx.curve_to(p1.x, p1.y, p2.x, p2.y, p3.x, p3.y);
                    last = p3;
                }
                PathEl::ClosePath => self.ctx.close_path(),
            }
        }
    }

    fn draw_image_inner(
        &mut self,
        image: &ImageSurface,
        src_rect: Option<Rect>,
        dst_rect: Rect,
        interp: InterpolationMode,
    ) {
        let src_rect = match src_rect {
            Some(src_rect) => src_rect,
            None => Size::new(image.get_width() as f64, image.get_height() as f64).to_rect(),
        };
        // Cairo returns an error if we try to paint an empty image, causing us to panic. We check if
        // either the source or destination is empty, and early-return if so.
        if src_rect.is_empty() || dst_rect.is_empty() {
            return;
        }

        let _ = self.with_save(|rc| {
            let surface_pattern = SurfacePattern::create(image);
            let filter = match interp {
                InterpolationMode::NearestNeighbor => Filter::Nearest,
                InterpolationMode::Bilinear => Filter::Bilinear,
            };
            surface_pattern.set_filter(filter);
            let scale_x = dst_rect.width() / src_rect.width();
            let scale_y = dst_rect.height() / src_rect.height();
            rc.clip(dst_rect);
            rc.ctx.translate(
                dst_rect.x0 - scale_x * src_rect.x0,
                dst_rect.y0 - scale_y * src_rect.y0,
            );
            rc.ctx.scale(scale_x, scale_y);
            rc.ctx.set_source(&surface_pattern);
            rc.ctx.paint();
            Ok(())
        });
    }
}

#[allow(deprecated)]
fn convert_line_cap(line_cap: LineCap) -> cairo::LineCap {
    match line_cap {
        LineCap::Butt => cairo::LineCap::Butt,
        LineCap::Round => cairo::LineCap::Round,
        LineCap::Square => cairo::LineCap::Square,
    }
}

fn convert_line_join(line_join: LineJoin) -> cairo::LineJoin {
    match line_join {
        LineJoin::Miter { .. } => cairo::LineJoin::Miter,
        LineJoin::Round => cairo::LineJoin::Round,
        LineJoin::Bevel => cairo::LineJoin::Bevel,
    }
}

fn byte_to_frac(byte: u32) -> f64 {
    ((byte & 255) as f64) * (1.0 / 255.0)
}

/// Can't implement RoundFrom here because both types belong to other crates.
fn affine_to_matrix(affine: Affine) -> Matrix {
    let a = affine.as_coeffs();
    Matrix {
        xx: a[0],
        yx: a[1],
        xy: a[2],
        yy: a[3],
        x0: a[4],
        y0: a[5],
    }
}

fn compute_blurred_rect(rect: Rect, radius: f64) -> (ImageSurface, Point) {
    let size = piet::util::size_for_blurred_rect(rect, radius);
    // TODO: maybe not panic on error (but likely to happen only in extreme cases such as OOM)
    let mut image =
        ImageSurface::create(Format::A8, size.width as i32, size.height as i32).unwrap();
    let stride = image.get_stride() as usize;
    let mut data = image.get_data().unwrap();
    let rect_exp = piet::util::compute_blurred_rect(rect, radius, stride, &mut *data);
    std::mem::drop(data);
    let origin = rect_exp.origin();
    (image, origin)
}

fn write_rgba(data: &mut [u8], column: usize, r: u8, g: u8, b: u8, a: u8) {
    // From the cairo docs for CAIRO_FORMAT_ARGB32:
    // > each pixel is a 32-bit quantity, with alpha in the upper 8 bits, then red,
    // > then green, then blue. The 32-bit quantities are stored native-endian.
    let (a, r, g, b) = (u32::from(a), u32::from(r), u32::from(g), u32::from(b));
    let pixel = a << 24 | r << 16 | g << 8 | b;

    data[4 * column..4 * (column + 1)].copy_from_slice(&pixel.to_ne_bytes());
}

fn write_rgb(data: &mut [u8], column: usize, r: u8, g: u8, b: u8) {
    // From the cairo docs for CAIRO_FORMAT_RGB24:
    //  each pixel is a 32-bit quantity, with the upper 8 bits unused.
    write_rgba(data, column, r, g, b, 0);
}
