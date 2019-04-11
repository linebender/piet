//! The CoreGraphics backend for the Piet 2D graphics abstraction.

use core_graphics::context::{CGContext, CGLineCap, CGLineJoin};
use core_graphics::image::CGImage;
use core_graphics::base::{CGFloat};

use kurbo::{PathEl, Rect, Affine, QuadBez, Vec2, Shape};

use piet::{
    new_error, Error, ErrorKind, LineCap, LineJoin, FillRule, Font, FontBuilder, Gradient, ImageFormat,
    InterpolationMode, RenderContext, RoundInto, StrokeStyle, Text, TextLayout, TextLayoutBuilder,
};

pub struct CoreGraphicsContext<'a> {
    // Cairo has this as Clone and with &self methods, but we do this to avoid
    // concurrency problems.
    ctx: &'a mut CGContext,
    text: CoreGraphicsText,
}

impl<'a> CoreGraphicsContext<'a> {
    pub fn new(ctx: &mut CGContext) -> CoreGraphicsContext {
        CoreGraphicsContext { ctx, text: CoreGraphicsText }
    }
}

pub enum Brush {
    Solid(u32),
}

pub struct CoreGraphicsFont;
impl Font for CoreGraphicsFont {
}

pub struct CoreGraphicsFontBuilder;

impl FontBuilder for CoreGraphicsFontBuilder {
    type Out = CoreGraphicsFont;

        fn build(self) -> Result<Self::Out, Error> {
panic!()
        }
}
pub struct CoreGraphicsLayout;

impl TextLayout for CoreGraphicsLayout {
    type Coord = CGFloat;
    fn width(&self) -> Self::Coord {
        panic!();
    }
}

pub struct CoreGraphicsTextLayoutBuilder {
}

impl TextLayoutBuilder for CoreGraphicsTextLayoutBuilder {
    type Out = CoreGraphicsLayout;

    fn build(self) -> Result<Self::Out, Error> {
        panic!()
    }
}

pub struct CoreGraphicsText;

impl Text for CoreGraphicsText {
    type Coord = CGFloat;
    type FontBuilder = CoreGraphicsFontBuilder;
    type Font = CoreGraphicsFont;
    type TextLayoutBuilder = CoreGraphicsTextLayoutBuilder;
    type TextLayout = CoreGraphicsLayout;

    fn new_font_by_name(&mut self, name: &str, size: impl RoundInto<Self::Coord>,) -> Result<Self::FontBuilder, Error> {
        panic!()
    }

       fn new_text_layout(
        &mut self,
        font: &Self::Font,
        text: &str,
    ) -> Result<Self::TextLayoutBuilder, Error> {
        panic!()
    }
}

// TODO: This cannot be used yet because the `piet::RenderContext` trait
// needs to expose a way to create stroke styles.
/*pub struct StrokeStyle {
    line_join: Option<CGLineJoin>,
    line_cap: Option<CGLineCap>,
    dash: Option<(Vec<f64>, f64)>,
    miter_limit: Option<f64>,
}

impl StrokeStyle {
    pub fn new() -> StrokeStyle {
        StrokeStyle {
            line_join: None,
            line_cap: None,
            dash: None,
            miter_limit: None,
        }
    }

    pub fn line_join(mut self, line_join: CGLineJoin) -> Self {
        self.line_join = Some(line_join);
        self
    }

    pub fn line_cap(mut self, line_cap: CGLineCap) -> Self {
        self.line_cap = Some(line_cap);
        self
    }

    pub fn dash(mut self, dashes: Vec<f64>, offset: f64) -> Self {
        self.dash = Some((dashes, offset));
        self
    }

    pub fn miter_limit(mut self, miter_limit: f64) -> Self {
        self.miter_limit = Some(miter_limit);
        self
    }
}*/

impl<'a> RenderContext for CoreGraphicsContext<'a> {
    type Point = Vec2;
    type Coord = f64;//XXX: this needs to be fixed for 32bit
    type Brush = Brush;
    type Text = CoreGraphicsText;
    type TextLayout = CoreGraphicsLayout;
    type Image = CGImage;
    //type StrokeStyle = StrokeStyle;

    fn clear(&mut self, rgb: u32) {
        self.ctx.set_rgb_fill_color(
            byte_to_frac(rgb >> 16),
            byte_to_frac(rgb >> 8),
            byte_to_frac(rgb),
            1.0,
        );
        self.ctx.fill_rect(self.ctx.clip_bounding_box());
    }

    fn solid_brush(&mut self, rgba: u32) -> Result<Brush, Error> {
        Ok(Brush::Solid(rgba))
    }

    fn gradient(&mut self, gradient: Gradient) -> Result<Brush, Error> {
        unimplemented!()
    }

    /// Fill a shape.
    fn fill(&mut self,
        shape: impl Shape,
        brush: &Self::Brush,
        fill_rule: FillRule,
    ) {
        self.set_path(shape);
        self.set_fill_brush(brush);
        match fill_rule {
            FillRule::NonZero => self.ctx.fill_path(),
            FillRule::EvenOdd => self.ctx.eo_fill_path(),
        }
    }

    fn clip(&mut self, shape: impl Shape, fill_rule: FillRule) {
        self.set_path(shape);
        match fill_rule {
            FillRule::NonZero => self.ctx.clip(),
            FillRule::EvenOdd => self.ctx.eo_clip(),
        }
    }

    fn stroke(
        &mut self,
        shape: impl Shape,
        brush: &Self::Brush,
        width: impl RoundInto<Self::Coord>,
        style: Option<&StrokeStyle>,
    ) {
        self.set_path(shape);
        self.set_stroke(width.round_into(), style);
        self.set_stroke_brush(brush);
        self.ctx.stroke_path();
    }

    fn text(&mut self) -> &mut Self::Text {
        &mut self.text
    }

    fn draw_text(
        &mut self,
        layout: &Self::TextLayout,
        pos: impl RoundInto<Self::Point>,
        brush: &Self::Brush,
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
        unimplemented!()
    }

    fn transform(&mut self, transform: Affine) {
        unimplemented!()
    }

    fn make_image(
        &mut self,
        width: usize,
        height: usize,
        buf: &[u8],
        format: ImageFormat,
    ) -> Result<Self::Image, Error> {
        unimplemented!()
    }

    fn draw_image(
                  &mut self,
                  image: &Self::Image,
                  rect: impl Into<Rect>,
                  interp: InterpolationMode,
                 ) {
        unimplemented!()
    }

    fn status(&mut self) -> Result<(), Error> {
        unimplemented!()
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
        let mut last = Vec2::default();
        for el in shape.to_bez_path(1e-3) {
            match el {
                PathEl::Moveto(p) => {
                    self.ctx.move_to_point(p.x, p.y);
                    last = p;
                }
                PathEl::Lineto(p) => {
                    self.ctx.add_line_to_point(p.x, p.y);
                    last = p;
                }
                PathEl::Quadto(p1, p2) => {
                    let q = QuadBez::new(last, p1, p2);
                    let c = q.raise();
                    self.ctx
                        .add_curve_to_point(c.p1.x, c.p1.y, c.p2.x, c.p2.y, p2.x, p2.y);
                    last = p2;
                }
                PathEl::Curveto(p1, p2, p3) => {
                    self.ctx.add_curve_to_point(p1.x, p1.y, p2.x, p2.y, p3.x, p3.y);
                    last = p3;
                }
                PathEl::Closepath => self.ctx.close_path(),
            }
        }
    }

}

fn byte_to_frac(byte: u32) -> f64 {
    ((byte & 255) as f64) * (1.0 / 255.0)
}
