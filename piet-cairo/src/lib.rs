//! The Cairo backend for the Piet 2D graphics abstraction.

use std::fmt;

use cairo::{
    BorrowError, Context, Filter, FontFace, FontOptions, FontSlant, FontWeight, Format,
    ImageSurface, Matrix, Pattern, PatternTrait, ScaledFont, Status, SurfacePattern,
};

use kurbo::{Affine, PathEl, QuadBez, Rect, Shape, Vec2};

use piet::{
    new_error, Error, ErrorKind, Factory, FillRule, Font, FontBuilder, ImageFormat,
    InterpolationMode, LineCap, LineJoin, RenderContext, RoundInto, StrokeStyle, TextLayout,
    TextLayoutBuilder,
};

pub struct CairoRenderContext<'a> {
    // Cairo has this as Clone and with &self methods, but we do this to avoid
    // concurrency problems.
    ctx: &'a mut Context,
    factory: CairoFactory,
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
            factory: CairoFactory,
        }
    }
}

pub enum Brush {
    Solid(u32),
}

/// Right now, we don't need any state, as the "toy text API" treats the
/// access to system font information as a global. This will change.
pub struct CairoFactory;

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

fn convert_fill_rule(fill_rule: piet::FillRule) -> cairo::FillRule {
    match fill_rule {
        piet::FillRule::NonZero => cairo::FillRule::Winding,
        piet::FillRule::EvenOdd => cairo::FillRule::EvenOdd,
    }
}

impl<'a> RenderContext for CairoRenderContext<'a> {
    /// Cairo mostly uses raw f64, so this is as convenient as anything.
    type Point = Vec2;
    type Coord = f64;
    type Brush = Brush;

    type Factory = CairoFactory;
    type TextLayout = CairoTextLayout;

    type Image = ImageSurface;

    fn clear(&mut self, rgb: u32) -> Result<(), Error> {
        self.ctx.set_source_rgb(
            byte_to_frac(rgb >> 16),
            byte_to_frac(rgb >> 8),
            byte_to_frac(rgb),
        );
        self.ctx.paint();
        self.status()
    }

    fn solid_brush(&mut self, rgba: u32) -> Result<Brush, Error> {
        Ok(Brush::Solid(rgba))
    }

    fn fill(
        &mut self,
        shape: impl Shape,
        brush: &Self::Brush,
        fill_rule: FillRule,
    ) -> Result<(), Error> {
        self.set_path(shape);
        self.set_brush(brush);
        self.ctx.set_fill_rule(convert_fill_rule(fill_rule));
        self.ctx.fill();
        self.status()
    }

    fn clip(&mut self, shape: impl Shape, fill_rule: FillRule) -> Result<(), Error> {
        self.set_path(shape);
        self.ctx.set_fill_rule(convert_fill_rule(fill_rule));
        self.ctx.clip();
        self.status()
    }

    fn stroke(
        &mut self,
        shape: impl Shape,
        brush: &Self::Brush,
        width: impl RoundInto<Self::Coord>,
        style: Option<&StrokeStyle>,
    ) -> Result<(), Error> {
        self.set_path(shape);
        self.set_stroke(width.round_into(), style);
        self.set_brush(brush);
        self.ctx.stroke();
        self.status()
    }

    fn factory(&mut self) -> &mut Self::Factory {
        &mut self.factory
    }

    fn draw_text(
        &mut self,
        layout: &Self::TextLayout,
        pos: impl RoundInto<Self::Point>,
        brush: &Self::Brush,
    ) -> Result<(), Error> {
        self.ctx.set_scaled_font(&layout.font);
        self.set_brush(brush);
        let pos = pos.round_into();
        self.ctx.move_to(pos.x, pos.y);
        self.ctx.show_text(&layout.text);
        self.status()
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
    ) -> Result<(), Error> {
        self.with_save(|rc| {
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
            rc.ctx.set_source(&Pattern::SurfacePattern(surface_pattern));
            rc.ctx.paint();
            rc.status()
        })
    }
}

impl Factory for CairoFactory {
    type Coord = f64;

    type Font = CairoFont;
    type FontBuilder = CairoFontBuilder;
    type TextLayout = CairoTextLayout;
    type TextLayoutBuilder = CairoTextLayoutBuilder;

    fn new_font_by_name(
        &mut self,
        name: &str,
        size: impl RoundInto<Self::Coord>,
    ) -> Result<Self::FontBuilder, Error> {
        Ok(CairoFontBuilder {
            family: name.to_owned(),
            size: size.round_into(),
            weight: FontWeight::Normal,
            slant: FontSlant::Normal,
        })
    }

    fn new_text_layout(
        &mut self,
        font: &Self::Font,
        text: &str,
    ) -> Result<Self::TextLayoutBuilder, Error> {
        let text_layout = CairoTextLayout {
            font: font.0.clone(),
            text: text.to_owned(),
        };
        Ok(CairoTextLayoutBuilder(text_layout))
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
        let mut last = Vec2::default();
        for el in shape.to_bez_path(1e-3) {
            match el {
                PathEl::Moveto(p) => {
                    self.ctx.move_to(p.x, p.y);
                    last = p;
                }
                PathEl::Lineto(p) => {
                    self.ctx.line_to(p.x, p.y);
                    last = p;
                }
                PathEl::Quadto(p1, p2) => {
                    let q = QuadBez::new(last, p1, p2);
                    let c = q.raise();
                    self.ctx
                        .curve_to(c.p1.x, c.p1.y, c.p2.x, c.p2.y, p2.x, p2.y);
                    last = p2;
                }
                PathEl::Curveto(p1, p2, p3) => {
                    self.ctx.curve_to(p1.x, p1.y, p2.x, p2.y, p3.x, p3.y);
                    last = p3;
                }
                PathEl::Closepath => self.ctx.close_path(),
            }
        }
    }

    fn status(&self) -> Result<(), Error> {
        let status = self.ctx.status();
        if status == Status::Success {
            Ok(())
        } else {
            let e: Box<dyn std::error::Error> = Box::new(WrappedStatus(status));
            Err(e.into())
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
    type Coord = f64;

    fn width(&self) -> f64 {
        self.font.text_extents(&self.text).width
    }
}
