//! The Cairo backend for the Piet 2D graphics abstraction.

use std::borrow::Cow;
use std::fmt;

use cairo::{
    BorrowError, Context, Filter, FontFace, FontOptions, FontSlant, FontWeight, Format,
    ImageSurface, Matrix, ScaledFont, Status, SurfacePattern,
};

use piet::kurbo::{Affine, PathEl, Point, QuadBez, Rect, Shape};

use piet::{
    new_error, Color, Error, ErrorKind, FixedGradient, Font, FontBuilder, ImageFormat,
    InterpolationMode, IntoBrush, LineCap, LineJoin, RenderContext, RoundInto, StrokeStyle, Text,
    TextLayout, TextLayoutBuilder, HitTestPoint, HitTestTextPosition, HitTestMetrics,
};

pub struct CairoRenderContext<'a> {
    // Cairo has this as Clone and with &self methods, but we do this to avoid
    // concurrency problems.
    ctx: &'a mut Context,
    text: CairoText,
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
            text: CairoText,
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
pub struct CairoText;

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

    type Text = CairoText;
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

impl CairoText {
    /// Create a new factory that satisfies the piet `Text` trait.
    ///
    /// No state is needed for now because the current implementation is just
    /// toy text, but that will change when proper text is implemented.
    pub fn new() -> CairoText {
        CairoText
    }
}

impl Text for CairoText {
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
    // TODO ask about f32/f64, u32/usize
    // TODO ask what exactly a text position is.
    // What text position should I report? the byte index or the char index?
    // If the second, I'll need to iterate over chars.
    fn hit_test_point(&self, point_x: f32, point_y: f32) -> HitTestPoint {
        let end_text_position = self.text.len() - 1;
        let end_codepoint_boundaries = self.get_codepoint_boundaries(end_text_position as u32);

        let start_text_position = 0;
        let start_codepoint_boundaries = self.get_codepoint_boundaries(start_text_position as u32);

        // first test beyond ends
        if point_x > end_codepoint_boundaries.end_x {
            return HitTestPoint {
                metrics: HitTestMetrics {
                    text_position: end_text_position as u32,
                    is_text: false,
                },
                is_inside: false,
                is_trailing_hit: true,
            }
        }
        if point_x < start_codepoint_boundaries.start_x {
            return HitTestPoint {
                metrics: HitTestMetrics {
                    text_position: start_text_position as u32,
                    is_text: false,
                },
                is_inside: false,
                is_trailing_hit: false,
            }
        }

        // then test the beginning
        if let Some(hit) = point_x_in_codepoint(point_x, &start_codepoint_boundaries) {
            return hit;
        }

        // then test the end
        if let Some(hit) = point_x_in_codepoint(point_x, &end_codepoint_boundaries) {
            return hit;
        }

        // Now that we know it's not beginning or end, begin binary search
        // We'll keep looping until there's a hit; there must be a hit, as we're searching
        // in a continuous range and we're using only trailing edges. unless there's something
        // funky that can happen TODO ask Raph about this.
        // also, I looped, but is it better to recurse? I prefer loop
        let mut search_start_idx = start_text_position;
        let mut search_end_idx = end_text_position;
        loop {
            // pick halfway point
            let current_idx = (search_end_idx - search_start_idx) / 2;

            let codepoint_boundaries = self.get_codepoint_boundaries(current_idx as u32);
            if let Some(hit) = point_x_in_codepoint(point_x, &codepoint_boundaries) {
                return hit;
            }

            // since it's not a hit, check if closer to start or finish
            // and move the appropriate search boundary
            if point_x < codepoint_boundaries.start_x {
                search_end_idx = codepoint_boundaries.start_idx as usize; // should this be -1?
            } else if point_x > codepoint_boundaries.end_x {
                search_start_idx = codepoint_boundaries.end_idx as usize; // should this be +1?
            }

            // TODO do I need to add a condition in case something goes terribly wrong
            // and search start idx crosses search end idx?
        }
    }

    // TODO substring slicing only works with ascii
    // Text position is treated as the ascii index; if it's not at a utf-8 boundary, we'll
    // have to step in one direction or another to find the boundary (just use a loop to move
    // until we get Some())
    fn hit_test_text_position(&self, text_position: u32, trailing: bool) -> Option<HitTestTextPosition> {
        // substring hack, from futurepaul/druid -> better-textbox
        let end = if trailing { text_position } else { text_position - 1};

        if let Some(substring) = self.text.get(0..end as usize) {
            // use innards of `width`, but substitute substring for text
            // TODO f32 from windows, f64 elsewhere?
            let point_x = self.font.text_extents(&substring).x_advance as f32;

            Some(HitTestTextPosition {
                point_x,
                point_y: 0.0,
                metrics: HitTestMetrics {
                    text_position,
                    is_text: true,
                },
            })
        } else {
            None
        }
    }
}
impl CairoTextLayout {
    // a text position here is just the byte index. should it be
    // measured by char?
    // it's an inclusive range for both idx and x_point
    fn get_codepoint_boundaries(&self, text_position: u32) -> CodePointBoundaries {
        let mut res = CodePointBoundaries::default();

        // given an index into a str, iterate forward and back to find the boundaries of the
        // char (unicode codepoint)

        // first do leading edge
        // if text position is 0, the leading edge defaults to 0
        if text_position != 0 {
            let mut leading_idx = text_position - 1; // looking at trailing edges only, not leading edg of current
            let leading_idx_trailing_hit =
                loop {
                    match self.hit_test_text_position(leading_idx as u32, true) {
                        Some(pos) => break pos,
                        None => leading_idx -= 1, // shouldn't be able to go off the front end?
                    }
                };

            res.start_idx = leading_idx + 1; // x uses trailing edge of previous idx, but starting idx should be part of codepoint
            res.start_x = leading_idx_trailing_hit.point_x;
        }

        // now trailing edge
        let mut trailing_idx = text_position;
        let trailing_idx_trailing_hit =
            loop {
                match self.hit_test_text_position(trailing_idx as u32, true) {
                    Some(pos) => break pos,
                    None => trailing_idx += 1, // shouldn't be able to go off the back end?
                }
            };

        res.end_idx = trailing_idx;
        res.end_x = trailing_idx_trailing_hit.point_x;

        res
    }
}

fn point_x_in_codepoint(point_x: f32, codepoint_boundaries: &CodePointBoundaries) -> Option<HitTestPoint> {
    let mut res = HitTestPoint::default();
    let start_x = codepoint_boundaries.start_x;
    let end_x = codepoint_boundaries.end_x;
    let start_idx = codepoint_boundaries.start_idx;
    let end_idx = codepoint_boundaries.end_idx;

    if point_x >= start_x && point_x <= end_x {
        // if inside, check which boundary it's closer to
        let is_trailing_hit = (end_x - point_x) > (point_x - start_x);

        res.is_inside= true;
        res.is_trailing_hit= is_trailing_hit; // TODO double check what this means?
        res.metrics.text_position = if is_trailing_hit { end_idx } else { start_idx };
        res.metrics.is_text = true;
        Some(res)
    } else {
        None
    }
}

#[derive(Debug, Default)]
struct CodePointBoundaries {
    start_idx: u32,
    end_idx: u32,
    start_x: f32,
    end_x: f32,
}
