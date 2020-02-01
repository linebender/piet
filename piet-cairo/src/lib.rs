//! The Cairo backend for the Piet 2D graphics abstraction.

mod grapheme;

use std::borrow::Cow;
use std::fmt;
use std::marker::PhantomData;

use cairo::{
    BorrowError, Context, Filter, FontFace, FontOptions, FontSlant, FontWeight, Format,
    ImageSurface, Matrix, ScaledFont, Status, SurfacePattern,
};

use piet::kurbo::{Affine, PathEl, Point, QuadBez, Rect, Shape};

use piet::{
    new_error, Color, Error, ErrorKind, FixedGradient, Font, FontBuilder, HitTestMetrics,
    HitTestPoint, HitTestTextPosition, ImageFormat, InterpolationMode, IntoBrush, LineCap,
    LineJoin, RenderContext, RoundInto, StrokeStyle, Text, TextLayout, TextLayoutBuilder,
};

use unicode_segmentation::UnicodeSegmentation;

use crate::grapheme::point_x_in_grapheme;

pub struct CairoRenderContext<'a> {
    // Cairo has this as Clone and with &self methods, but we do this to avoid
    // concurrency problems.
    ctx: &'a mut Context,
    text: CairoText<'a>,
}

impl<'a> CairoRenderContext<'a> {
    /// Create a new Cairo back-end.
    ///
    /// At the moment, it uses the "toy text API" for text layout, but when
    /// we change to a more sophisticated text layout approach, we'll probably
    /// need a factory for that as an additional argument.
    pub fn new(ctx: &mut Context) -> CairoRenderContext {
        CairoRenderContext {
            ctx,
            text: CairoText(PhantomData),
        }
    }
}

#[derive(Clone)]
pub enum Brush {
    Solid(u32),
    Linear(cairo::LinearGradient),
    Radial(cairo::RadialGradient),
}

/// Right now, we don't need any state, as the "toy text API" treats the
/// access to system font information as a global. This will change.
// we use a phantom lifetime here to match the API of the d2d backend,
// and the likely API of something with access to system font information.
pub struct CairoText<'a>(PhantomData<&'a ()>);

pub struct CairoFont(ScaledFont);

pub struct CairoFontBuilder {
    family: String,
    weight: FontWeight,
    slant: FontSlant,
    size: f64,
}

pub struct CairoTextLayout {
    font: ScaledFont,
    text: String,
}

pub struct CairoTextLayoutBuilder(CairoTextLayout);

#[derive(Debug)]
struct WrappedStatus(Status);

impl fmt::Display for WrappedStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Cairo error: {:?}", self.0)
    }
}

impl std::error::Error for WrappedStatus {}

trait WrapError<T> {
    fn wrap(self) -> Result<T, Error>;
}

// Discussion question: a blanket impl here should be pretty doable.

impl<T> WrapError<T> for Result<T, BorrowError> {
    fn wrap(self) -> Result<T, Error> {
        self.map_err(|e| {
            let e: Box<dyn std::error::Error> = Box::new(e);
            e.into()
        })
    }
}

impl<T> WrapError<T> for Result<T, Status> {
    fn wrap(self) -> Result<T, Error> {
        self.map_err(|e| {
            let e: Box<dyn std::error::Error> = Box::new(WrappedStatus(e));
            e.into()
        })
    }
}

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

    type Text = CairoText<'a>;
    type TextLayout = CairoTextLayout;

    type Image = ImageSurface;

    fn status(&mut self) -> Result<(), Error> {
        let status = self.ctx.status();
        if status == Status::Success {
            Ok(())
        } else {
            let e: Box<dyn std::error::Error> = Box::new(WrappedStatus(status));
            Err(e.into())
        }
    }

    fn clear(&mut self, color: Color) {
        let rgba = color.as_rgba_u32();
        self.ctx.set_source_rgb(
            byte_to_frac(rgba >> 24),
            byte_to_frac(rgba >> 16),
            byte_to_frac(rgba >> 8),
        );
        self.ctx.paint();
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

    fn draw_text(
        &mut self,
        layout: &Self::TextLayout,
        pos: impl Into<Point>,
        brush: &impl IntoBrush<Self>,
    ) {
        // TODO: bounding box for text
        let brush = brush.make_brush(self, || Rect::ZERO);
        self.ctx.set_scaled_font(&layout.font);
        self.set_brush(&*brush);
        let pos = pos.into();
        self.ctx.move_to(pos.x, pos.y);
        self.ctx.show_text(&layout.text);
    }

    fn save(&mut self) -> Result<(), Error> {
        self.ctx.save();
        self.status()
    }

    fn restore(&mut self) -> Result<(), Error> {
        self.ctx.restore();
        self.status()
    }

    fn finish(&mut self) -> Result<(), Error> {
        self.status()
    }

    fn transform(&mut self, transform: Affine) {
        self.ctx.transform(affine_to_matrix(transform));
    }

    fn current_transform(&self) -> Affine {
        matrix_to_affine(self.ctx.get_matrix())
    }

    fn make_image(
        &mut self,
        width: usize,
        height: usize,
        buf: &[u8],
        format: ImageFormat,
    ) -> Result<Self::Image, Error> {
        let cairo_fmt = match format {
            ImageFormat::Rgb => Format::Rgb24,
            ImageFormat::RgbaSeparate | ImageFormat::RgbaPremul => Format::ARgb32,
            _ => return Err(new_error(ErrorKind::NotSupported)),
        };
        let mut image = ImageSurface::create(cairo_fmt, width as i32, height as i32).wrap()?;
        // Confident no borrow errors because we just created it.
        let bytes_per_pixel = format.bytes_per_pixel();
        let bytes_per_row = width * bytes_per_pixel;
        let stride = image.get_stride() as usize;
        {
            let mut data = image.get_data().wrap()?;
            for y in 0..height {
                let src_off = y * bytes_per_row;
                let dst_off = y * stride;
                match format {
                    ImageFormat::Rgb => {
                        for x in 0..width {
                            data[dst_off + x * 4 + 0] = buf[src_off + x * 3 + 2];
                            data[dst_off + x * 4 + 1] = buf[src_off + x * 3 + 1];
                            data[dst_off + x * 4 + 2] = buf[src_off + x * 3 + 0];
                        }
                    }
                    ImageFormat::RgbaPremul => {
                        // It's annoying that Cairo exposes only ARGB. Ah well. Let's
                        // hope that LLVM generates pretty good code for this.
                        // TODO: consider adding BgraPremul format.
                        for x in 0..width {
                            data[dst_off + x * 4 + 0] = buf[src_off + x * 4 + 2];
                            data[dst_off + x * 4 + 1] = buf[src_off + x * 4 + 1];
                            data[dst_off + x * 4 + 2] = buf[src_off + x * 4 + 0];
                            data[dst_off + x * 4 + 3] = buf[src_off + x * 4 + 3];
                        }
                    }
                    ImageFormat::RgbaSeparate => {
                        fn premul(x: u8, a: u8) -> u8 {
                            let y = (x as u16) * (a as u16);
                            ((y + (y >> 8) + 0x80) >> 8) as u8
                        }
                        for x in 0..width {
                            let a = buf[src_off + x * 4 + 3];
                            data[dst_off + x * 4 + 0] = premul(buf[src_off + x * 4 + 2], a);
                            data[dst_off + x * 4 + 1] = premul(buf[src_off + x * 4 + 1], a);
                            data[dst_off + x * 4 + 2] = premul(buf[src_off + x * 4 + 0], a);
                            data[dst_off + x * 4 + 3] = a;
                        }
                    }
                    _ => return Err(new_error(ErrorKind::NotSupported)),
                }
            }
        }
        Ok(image)
    }

    fn draw_image(
        &mut self,
        image: &Self::Image,
        rect: impl Into<Rect>,
        interp: InterpolationMode,
    ) {
        let _ = self.with_save(|rc| {
            let surface_pattern = SurfacePattern::create(image);
            let filter = match interp {
                InterpolationMode::NearestNeighbor => Filter::Nearest,
                InterpolationMode::Bilinear => Filter::Bilinear,
            };
            surface_pattern.set_filter(filter);
            let rect = rect.into();
            rc.ctx.translate(rect.x0, rect.y0);
            rc.ctx.scale(
                rect.width() / (image.get_width() as f64),
                rect.height() / (image.get_height() as f64),
            );
            rc.ctx.set_source(&surface_pattern);
            rc.ctx.paint();
            Ok(())
        });
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

impl<'a> CairoText<'a> {
    /// Create a new factory that satisfies the piet `Text` trait.
    ///
    /// No state is needed for now because the current implementation is just
    /// toy text, but that will change when proper text is implemented.
    pub fn new() -> CairoText<'a> {
        CairoText(PhantomData)
    }
}

impl<'a> Text for CairoText<'a> {
    type Font = CairoFont;
    type FontBuilder = CairoFontBuilder;
    type TextLayout = CairoTextLayout;
    type TextLayoutBuilder = CairoTextLayoutBuilder;

    fn new_font_by_name(&mut self, name: &str, size: f64) -> Self::FontBuilder {
        CairoFontBuilder {
            family: name.to_owned(),
            size: size.round_into(),
            weight: FontWeight::Normal,
            slant: FontSlant::Normal,
        }
    }

    fn new_text_layout(&mut self, font: &Self::Font, text: &str) -> Self::TextLayoutBuilder {
        let text_layout = CairoTextLayout {
            font: font.0.clone(),
            text: text.to_owned(),
        };
        CairoTextLayoutBuilder(text_layout)
    }
}

fn convert_line_cap(line_cap: LineCap) -> cairo::LineCap {
    match line_cap {
        LineCap::Butt => cairo::LineCap::Butt,
        LineCap::Round => cairo::LineCap::Round,
        LineCap::Square => cairo::LineCap::Square,
    }
}

fn convert_line_join(line_join: LineJoin) -> cairo::LineJoin {
    match line_join {
        LineJoin::Miter => cairo::LineJoin::Miter,
        LineJoin::Round => cairo::LineJoin::Round,
        LineJoin::Bevel => cairo::LineJoin::Bevel,
    }
}

impl<'a> CairoRenderContext<'a> {
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
            None => self.ctx.set_dash(&[], 0.0),
            Some((dashes, offset)) => self.ctx.set_dash(dashes, *offset),
        }
    }

    fn set_path(&mut self, shape: impl Shape) {
        // This shouldn't be necessary, we always leave the context in no-path
        // state. But just in case, and it should be harmless.
        self.ctx.new_path();
        let mut last = Point::ZERO;
        for el in shape.to_bez_path(1e-3) {
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

fn matrix_to_affine(matrix: Matrix) -> Affine {
    Affine::new([
        matrix.xx, matrix.yx, matrix.xy, matrix.yy, matrix.x0, matrix.y0,
    ])
}

fn scale_matrix(scale: f64) -> Matrix {
    Matrix {
        xx: scale,
        yx: 0.0,
        xy: 0.0,
        yy: scale,
        x0: 0.0,
        y0: 0.0,
    }
}

impl FontBuilder for CairoFontBuilder {
    type Out = CairoFont;

    fn build(self) -> Result<Self::Out, Error> {
        let font_face = FontFace::toy_create(&self.family, self.slant, self.weight);
        let font_matrix = scale_matrix(self.size);
        let ctm = scale_matrix(1.0);
        let options = FontOptions::default();
        let scaled_font = ScaledFont::new(&font_face, &font_matrix, &ctm, &options);
        Ok(CairoFont(scaled_font))
    }
}

impl Font for CairoFont {}

impl TextLayoutBuilder for CairoTextLayoutBuilder {
    type Out = CairoTextLayout;

    fn build(self) -> Result<Self::Out, Error> {
        Ok(self.0)
    }
}

impl TextLayout for CairoTextLayout {
    fn width(&self) -> f64 {
        self.font.text_extents(&self.text).x_advance
    }

    // first assume one line.
    // TODO do with lines
    fn hit_test_point(&self, point: Point) -> HitTestPoint {
        // internal logic is using grapheme clusters, but return the text position associated
        // with the border of the grapheme cluster.

        // null case
        if self.text.len() == 0 {
            return HitTestPoint::default();
        }

        // get bounds
        // TODO handle if string is not null yet count is 0?
        let end = UnicodeSegmentation::graphemes(self.text.as_str(), true).count() - 1;
        let end_bounds = match self.get_grapheme_boundaries(end) {
            Some(bounds) => bounds,
            None => return HitTestPoint::default(),
        };

        let start = 0;
        let start_bounds = match self.get_grapheme_boundaries(start) {
            Some(bounds) => bounds,
            None => return HitTestPoint::default(),
        };

        // first test beyond ends
        if point.x > end_bounds.trailing {
            let mut res = HitTestPoint::default();
            res.metrics.text_position = self.text.len();
            return res;
        }
        if point.x <= start_bounds.leading {
            return HitTestPoint::default();
        }

        // then test the beginning and end (common cases)
        if let Some(hit) = point_x_in_grapheme(point.x, &start_bounds) {
            return hit;
        }
        if let Some(hit) = point_x_in_grapheme(point.x, &end_bounds) {
            return hit;
        }

        // Now that we know it's not beginning or end, begin binary search.
        // Iterative style
        let mut left = start;
        let mut right = end;
        loop {
            // pick halfway point
            let middle = left + ((right - left) / 2);

            let grapheme_bounds = match self.get_grapheme_boundaries(middle) {
                Some(bounds) => bounds,
                None => return HitTestPoint::default(),
            };

            if let Some(hit) = point_x_in_grapheme(point.x, &grapheme_bounds) {
                return hit;
            }

            // since it's not a hit, check if closer to start or finish
            // and move the appropriate search boundary
            if point.x < grapheme_bounds.leading {
                right = middle;
            } else if point.x > grapheme_bounds.trailing {
                left = middle + 1;
            } else {
                unreachable!("hit_test_point conditional is exhaustive");
            }
        }
    }

    fn hit_test_text_position(&self, text_position: usize) -> Option<HitTestTextPosition> {
        // Using substrings, but now with unicode grapheme awareness

        let text_len = self.text.len();

        if text_position == 0 {
            return Some(HitTestTextPosition::default());
        }

        if text_position as usize >= text_len {
            return Some(HitTestTextPosition {
                point: Point {
                    x: self.font.text_extents(&self.text).x_advance,
                    y: 0.0,
                },
                metrics: HitTestMetrics {
                    text_position: text_len,
                },
            });
        }

        // Already checked that text_position > 0 and text_position < count.
        // If text position is not at a grapheme boundary, use the text position of current
        // grapheme cluster. But return the original text position
        // Use the indices (byte offset, which for our purposes = utf8 code units).
        let grapheme_indices = UnicodeSegmentation::grapheme_indices(self.text.as_str(), true)
            .take_while(|(byte_idx, _s)| text_position >= *byte_idx);

        if let Some((byte_idx, _s)) = grapheme_indices.last() {
            let point_x = self.font.text_extents(&self.text[0..byte_idx]).x_advance;

            Some(HitTestTextPosition {
                point: Point { x: point_x, y: 0.0 },
                metrics: HitTestMetrics {
                    text_position: text_position,
                },
            })
        } else {
            // iterated to end boundary
            Some(HitTestTextPosition {
                point: Point {
                    x: self.font.text_extents(&self.text).x_advance,
                    y: 0.0,
                },
                metrics: HitTestMetrics {
                    text_position: text_len,
                },
            })
        }
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    use piet::TextLayout;

    // - x: calculated value
    // - target: f64
    // - tolerance: in f64
    fn assert_close_to(x: f64, target: f64, tolerance: f64) {
        let min = target - tolerance;
        let max = target + tolerance;
        println!("x: {}, target: {}", x, target);
        assert!(x <= max && x >= min);
    }

    #[test]
    fn test_hit_test_text_position_basic() {
        let mut text_layout = CairoText::new();

        let input = "piet text!";
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();

        let layout = text_layout
            .new_text_layout(&font, &input[0..4])
            .build()
            .unwrap();
        let piet_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, &input[0..3])
            .build()
            .unwrap();
        let pie_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, &input[0..2])
            .build()
            .unwrap();
        let pi_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, &input[0..1])
            .build()
            .unwrap();
        let p_width = layout.width();

        let layout = text_layout.new_text_layout(&font, "").build().unwrap();
        let null_width = layout.width();

        let full_layout = text_layout.new_text_layout(&font, input).build().unwrap();
        let full_width = full_layout.width();

        assert_close_to(
            full_layout.hit_test_text_position(4).unwrap().point.x as f64,
            piet_width,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(3).unwrap().point.x as f64,
            pie_width,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(2).unwrap().point.x as f64,
            pi_width,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(1).unwrap().point.x as f64,
            p_width,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(0).unwrap().point.x as f64,
            null_width,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(10).unwrap().point.x as f64,
            full_width,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(11).unwrap().point.x as f64,
            full_width,
            3.0,
        );
        assert_eq!(
            full_layout
                .hit_test_text_position(11)
                .unwrap()
                .metrics
                .text_position,
            10
        )
    }

    #[test]
    fn test_hit_test_text_position_complex_0() {
        let input = "é";
        assert_eq!(input.len(), 2);

        let mut text_layout = CairoText::new();
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout.new_text_layout(&font, input).build().unwrap();

        assert_close_to(layout.hit_test_text_position(0).unwrap().point.x, 0.0, 3.0);
        assert_close_to(
            layout.hit_test_text_position(2).unwrap().point.x,
            layout.width(),
            3.0,
        );

        // note code unit not at grapheme boundary
        // This one panics in d2d because this is not a code unit boundary.
        // But it works here! Harder to deal with this right now, since unicode-segmentation
        // doesn't give code point offsets.
        assert_close_to(layout.hit_test_text_position(1).unwrap().point.x, 0.0, 3.0);
        assert_eq!(
            layout
                .hit_test_text_position(1)
                .unwrap()
                .metrics
                .text_position,
            1
        );

        // unicode segmentation is wrong on this one for now.
        //let input = "🤦\u{1f3fc}\u{200d}\u{2642}\u{fe0f}";

        //let mut text_layout = D2DText::new();
        //let font = text_layout.new_font_by_name("sans-serif", 12.0).build().unwrap();
        //let layout = text_layout.new_text_layout(&font, input).build().unwrap();

        //assert_eq!(input.graphemes(true).count(), 1);
        //assert_eq!(layout.hit_test_text_position(0, true).map(|p| p.point_x as f64), Some(layout.width()));
        //assert_eq!(input.len(), 17);

        let input = "\u{0023}\u{FE0F}\u{20E3}"; // #️⃣
        assert_eq!(input.len(), 7);
        assert_eq!(input.chars().count(), 3);

        let mut text_layout = CairoText::new();
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout.new_text_layout(&font, input).build().unwrap();

        assert_close_to(layout.hit_test_text_position(0).unwrap().point.x, 0.0, 3.0);
        assert_close_to(
            layout.hit_test_text_position(7).unwrap().point.x,
            layout.width(),
            3.0,
        );

        // note code unit not at grapheme boundary
        assert_close_to(layout.hit_test_text_position(1).unwrap().point.x, 0.0, 3.0);
        assert_eq!(
            layout
                .hit_test_text_position(1)
                .unwrap()
                .metrics
                .text_position,
            1
        );
    }

    #[test]
    fn test_hit_test_text_position_complex_1() {
        // Notes on this input:
        // 6 code points
        // 7 utf-16 code units (1/1/1/1/1/2)
        // 14 utf-8 code units (2/1/3/3/1/4)
        // 4 graphemes
        let input = "é\u{0023}\u{FE0F}\u{20E3}1\u{1D407}"; // #️⃣,, 𝐇
        assert_eq!(input.len(), 14);

        let mut text_layout = CairoText::new();
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout.new_text_layout(&font, input).build().unwrap();

        let test_layout_0 = text_layout
            .new_text_layout(&font, &input[0..2])
            .build()
            .unwrap();
        let test_layout_1 = text_layout
            .new_text_layout(&font, &input[0..9])
            .build()
            .unwrap();
        let test_layout_2 = text_layout
            .new_text_layout(&font, &input[0..10])
            .build()
            .unwrap();

        // Note: text position is in terms of utf8 code units
        assert_close_to(layout.hit_test_text_position(0).unwrap().point.x, 0.0, 3.0);
        assert_close_to(
            layout.hit_test_text_position(2).unwrap().point.x,
            test_layout_0.width(),
            3.0,
        );
        assert_close_to(
            layout.hit_test_text_position(9).unwrap().point.x,
            test_layout_1.width(),
            3.0,
        );
        assert_close_to(
            layout.hit_test_text_position(10).unwrap().point.x,
            test_layout_2.width(),
            3.0,
        );
        assert_close_to(
            layout.hit_test_text_position(14).unwrap().point.x,
            layout.width(),
            3.0,
        );

        // Code point boundaries, but not grapheme boundaries.
        // Width should stay at the current grapheme boundary.
        assert_close_to(
            layout.hit_test_text_position(3).unwrap().point.x,
            test_layout_0.width(),
            3.0,
        );
        assert_eq!(
            layout
                .hit_test_text_position(3)
                .unwrap()
                .metrics
                .text_position,
            3
        );
        assert_close_to(
            layout.hit_test_text_position(6).unwrap().point.x,
            test_layout_0.width(),
            3.0,
        );
        assert_eq!(
            layout
                .hit_test_text_position(6)
                .unwrap()
                .metrics
                .text_position,
            6
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_hit_test_point_basic_0() {
        let mut text_layout = CairoText::new();

        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout
            .new_text_layout(&font, "piet text!")
            .build()
            .unwrap();
        println!("text pos 4: {:?}", layout.hit_test_text_position(4)); // 23.0
        println!("text pos 5: {:?}", layout.hit_test_text_position(5)); // 27.0

        // test hit test point
        // all inside
        let pt = layout.hit_test_point(Point::new(22.5, 0.0));
        assert_eq!(pt.metrics.text_position, 4);
        let pt = layout.hit_test_point(Point::new(23.0, 0.0));
        assert_eq!(pt.metrics.text_position, 4);
        let pt = layout.hit_test_point(Point::new(25.0, 0.0));
        assert_eq!(pt.metrics.text_position, 5);
        let pt = layout.hit_test_point(Point::new(26.0, 0.0));
        assert_eq!(pt.metrics.text_position, 5);
        let pt = layout.hit_test_point(Point::new(27.0, 0.0));
        assert_eq!(pt.metrics.text_position, 5);
        let pt = layout.hit_test_point(Point::new(28.0, 0.0));
        assert_eq!(pt.metrics.text_position, 5);

        // outside
        println!("layout_width: {:?}", layout.width()); // 56.0

        let pt = layout.hit_test_point(Point::new(56.0, 0.0));
        assert_eq!(pt.metrics.text_position, 10); // last text position
        assert_eq!(pt.is_inside, true);

        let pt = layout.hit_test_point(Point::new(57.0, 0.0));
        assert_eq!(pt.metrics.text_position, 10); // last text position
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(-1.0, 0.0));
        assert_eq!(pt.metrics.text_position, 0); // first text position
        assert_eq!(pt.is_inside, false);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_hit_test_point_basic_0() {
        let mut text_layout = CairoText::new();

        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout
            .new_text_layout(&font, "piet text!")
            .build()
            .unwrap();
        println!("text pos 4: {:?}", layout.hit_test_text_position(4)); // 19.34765625
        println!("text pos 5: {:?}", layout.hit_test_text_position(5)); // 22.681640625

        // test hit test point
        // all inside
        let pt = layout.hit_test_point(Point::new(19.0, 0.0));
        assert_eq!(pt.metrics.text_position, 4);
        let pt = layout.hit_test_point(Point::new(20.0, 0.0));
        assert_eq!(pt.metrics.text_position, 4);
        let pt = layout.hit_test_point(Point::new(21.0, 0.0));
        assert_eq!(pt.metrics.text_position, 4);
        let pt = layout.hit_test_point(Point::new(22.0, 0.0));
        assert_eq!(pt.metrics.text_position, 5);
        let pt = layout.hit_test_point(Point::new(23.0, 0.0));
        assert_eq!(pt.metrics.text_position, 5);

        // outside
        println!("layout_width: {:?}", layout.width()); //45.357421875

        let pt = layout.hit_test_point(Point::new(45.0, 0.0));
        assert_eq!(pt.metrics.text_position, 10); // last text position
        assert_eq!(pt.is_inside, true);

        let pt = layout.hit_test_point(Point::new(46.0, 0.0));
        assert_eq!(pt.metrics.text_position, 10); // last text position
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(-1.0, 0.0));
        assert_eq!(pt.metrics.text_position, 0); // first text position
        assert_eq!(pt.is_inside, false);
    }

    #[test]
    #[cfg(target_os = "linux")]
    // for testing that 'middle' assignment in binary search is correct
    fn test_hit_test_point_basic_1() {
        let mut text_layout = CairoText::new();

        // base condition, one grapheme
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout.new_text_layout(&font, "t").build().unwrap();
        println!("text pos 1: {:?}", layout.hit_test_text_position(1)); // 5.0

        // two graphemes (to check that middle moves)
        let pt = layout.hit_test_point(Point::new(1.0, 0.0));
        assert_eq!(pt.metrics.text_position, 0);

        let layout = text_layout.new_text_layout(&font, "te").build().unwrap();
        println!("text pos 1: {:?}", layout.hit_test_text_position(1)); // 5.0
        println!("text pos 2: {:?}", layout.hit_test_text_position(2)); // 12.0

        let pt = layout.hit_test_point(Point::new(1.0, 0.0));
        assert_eq!(pt.metrics.text_position, 0);
        let pt = layout.hit_test_point(Point::new(4.0, 0.0));
        assert_eq!(pt.metrics.text_position, 1);
        let pt = layout.hit_test_point(Point::new(6.0, 0.0));
        assert_eq!(pt.metrics.text_position, 1);
        let pt = layout.hit_test_point(Point::new(11.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
    }

    #[test]
    #[cfg(target_os = "macos")]
    // for testing that 'middle' assignment in binary search is correct
    fn test_hit_test_point_basic_1() {
        let mut text_layout = CairoText::new();

        // base condition, one grapheme
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout.new_text_layout(&font, "t").build().unwrap();
        println!("text pos 1: {:?}", layout.hit_test_text_position(1)); // 5.0

        // two graphemes (to check that middle moves)
        let pt = layout.hit_test_point(Point::new(1.0, 0.0));
        assert_eq!(pt.metrics.text_position, 0);

        let layout = text_layout.new_text_layout(&font, "te").build().unwrap();
        println!("text pos 1: {:?}", layout.hit_test_text_position(1)); // 5.0
        println!("text pos 2: {:?}", layout.hit_test_text_position(2)); // 12.0

        let pt = layout.hit_test_point(Point::new(1.0, 0.0));
        assert_eq!(pt.metrics.text_position, 0);
        let pt = layout.hit_test_point(Point::new(4.0, 0.0));
        assert_eq!(pt.metrics.text_position, 1);
        let pt = layout.hit_test_point(Point::new(6.0, 0.0));
        assert_eq!(pt.metrics.text_position, 1);
        let pt = layout.hit_test_point(Point::new(11.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_hit_test_point_complex_0() {
        // Notes on this input:
        // 6 code points
        // 7 utf-16 code units (1/1/1/1/1/2)
        // 14 utf-8 code units (2/1/3/3/1/4)
        // 4 graphemes
        let input = "é\u{0023}\u{FE0F}\u{20E3}1\u{1D407}"; // #️⃣,, 𝐇

        let mut text_layout = CairoText::new();
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout.new_text_layout(&font, input).build().unwrap();
        //println!("text pos 2: {:?}", layout.hit_test_text_position(2)); // 6.99999999
        //println!("text pos 9: {:?}", layout.hit_test_text_position(9)); // 24.0
        //println!("text pos 10: {:?}", layout.hit_test_text_position(10)); // 32.0
        //println!("text pos 14: {:?}", layout.hit_test_text_position(14)); // 39.0, line width

        let pt = layout.hit_test_point(Point::new(2.0, 0.0));
        assert_eq!(pt.metrics.text_position, 0);
        let pt = layout.hit_test_point(Point::new(4.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
        let pt = layout.hit_test_point(Point::new(7.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
        let pt = layout.hit_test_point(Point::new(10.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
        let pt = layout.hit_test_point(Point::new(14.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
        let pt = layout.hit_test_point(Point::new(18.0, 0.0));
        assert_eq!(pt.metrics.text_position, 9);
        let pt = layout.hit_test_point(Point::new(23.0, 0.0));
        assert_eq!(pt.metrics.text_position, 9);
        let pt = layout.hit_test_point(Point::new(26.0, 0.0));
        assert_eq!(pt.metrics.text_position, 9);
        let pt = layout.hit_test_point(Point::new(29.0, 0.0));
        assert_eq!(pt.metrics.text_position, 10);
        let pt = layout.hit_test_point(Point::new(32.0, 0.0));
        assert_eq!(pt.metrics.text_position, 10);
        let pt = layout.hit_test_point(Point::new(35.5, 0.0));
        assert_eq!(pt.metrics.text_position, 14);
        let pt = layout.hit_test_point(Point::new(38.0, 0.0));
        assert_eq!(pt.metrics.text_position, 14);
        let pt = layout.hit_test_point(Point::new(40.0, 0.0));
        assert_eq!(pt.metrics.text_position, 14);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_hit_test_point_complex_0() {
        // Notes on this input:
        // 6 code points
        // 7 utf-16 code units (1/1/1/1/1/2)
        // 14 utf-8 code units (2/1/3/3/1/4)
        // 4 graphemes
        let input = "é\u{0023}\u{FE0F}\u{20E3}1\u{1D407}"; // #️⃣,, 𝐇

        let mut text_layout = CairoText::new();
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout.new_text_layout(&font, input).build().unwrap();
        println!("text pos 2: {:?}", layout.hit_test_text_position(2)); // 6.673828125
        println!("text pos 9: {:?}", layout.hit_test_text_position(9)); // 28.55859375
        println!("text pos 10: {:?}", layout.hit_test_text_position(10)); // 35.232421875
        println!("text pos 14: {:?}", layout.hit_test_text_position(14)); // 42.8378905, line width

        let pt = layout.hit_test_point(Point::new(2.0, 0.0));
        assert_eq!(pt.metrics.text_position, 0);
        let pt = layout.hit_test_point(Point::new(4.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
        let pt = layout.hit_test_point(Point::new(7.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
        let pt = layout.hit_test_point(Point::new(10.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
        let pt = layout.hit_test_point(Point::new(14.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
        let pt = layout.hit_test_point(Point::new(18.0, 0.0));
        assert_eq!(pt.metrics.text_position, 9);
        let pt = layout.hit_test_point(Point::new(23.0, 0.0));
        assert_eq!(pt.metrics.text_position, 9);
        let pt = layout.hit_test_point(Point::new(26.0, 0.0));
        assert_eq!(pt.metrics.text_position, 9);
        let pt = layout.hit_test_point(Point::new(29.0, 0.0));
        assert_eq!(pt.metrics.text_position, 9);
        let pt = layout.hit_test_point(Point::new(32.0, 0.0));
        assert_eq!(pt.metrics.text_position, 10);
        let pt = layout.hit_test_point(Point::new(35.5, 0.0));
        assert_eq!(pt.metrics.text_position, 10);
        let pt = layout.hit_test_point(Point::new(38.0, 0.0));
        assert_eq!(pt.metrics.text_position, 10);
        let pt = layout.hit_test_point(Point::new(40.0, 0.0));
        assert_eq!(pt.metrics.text_position, 14);
        let pt = layout.hit_test_point(Point::new(43.0, 0.0));
        assert_eq!(pt.metrics.text_position, 14);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_hit_test_point_complex_1() {
        // this input caused an infinite loop in the binary search when test position
        // > 21.0 && < 28.0
        //
        // This corresponds to the char 'y' in the input.
        let input = "tßßypi";

        let mut text_layout = CairoText::new();
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout.new_text_layout(&font, input).build().unwrap();
        println!("text pos 0: {:?}", layout.hit_test_text_position(0)); // 0.0
        println!("text pos 1: {:?}", layout.hit_test_text_position(1)); // 5.0
        println!("text pos 2: {:?}", layout.hit_test_text_position(2)); // 5.0
        println!("text pos 3: {:?}", layout.hit_test_text_position(3)); // 13.0
        println!("text pos 4: {:?}", layout.hit_test_text_position(4)); // 13.0
        println!("text pos 5: {:?}", layout.hit_test_text_position(5)); // 21.0
        println!("text pos 6: {:?}", layout.hit_test_text_position(6)); // 28.0
        println!("text pos 7: {:?}", layout.hit_test_text_position(7)); // 36.0
        println!("text pos 8: {:?}", layout.hit_test_text_position(8)); // 39.0, end

        let pt = layout.hit_test_point(Point::new(27.0, 0.0));
        assert_eq!(pt.metrics.text_position, 6);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_hit_test_point_complex_1() {
        // this input caused an infinite loop in the binary search when test position
        // > 21.0 && < 28.0
        //
        // This corresponds to the char 'y' in the input.
        let input = "tßßypi";

        let mut text_layout = CairoText::new();
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout.new_text_layout(&font, input).build().unwrap();
        println!("text pos 0: {:?}", layout.hit_test_text_position(0)); // 0.0
        println!("text pos 1: {:?}", layout.hit_test_text_position(1)); // 5.0
        println!("text pos 2: {:?}", layout.hit_test_text_position(2)); // 5.0
        println!("text pos 3: {:?}", layout.hit_test_text_position(3)); // 13.0
        println!("text pos 4: {:?}", layout.hit_test_text_position(4)); // 13.0
        println!("text pos 5: {:?}", layout.hit_test_text_position(5)); // 21.0
        println!("text pos 6: {:?}", layout.hit_test_text_position(6)); // 28.0
        println!("text pos 7: {:?}", layout.hit_test_text_position(7)); // 36.0
        println!("text pos 8: {:?}", layout.hit_test_text_position(8)); // 39.0, end

        let pt = layout.hit_test_point(Point::new(27.0, 0.0));
        assert_eq!(pt.metrics.text_position, 6);
    }
}
