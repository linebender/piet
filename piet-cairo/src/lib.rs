//! The Cairo backend for the Piet 2D graphics abstraction.

use cairo::Context;

use kurbo::{PathEl, QuadBez, Vec2};

use piet::{RenderContext, RoundInto};

pub struct CairoRenderContext<'a> {
    // Cairo has this as Clone and with &self methods, but we do this to avoid
    // concurrency problems.
    ctx: &'a mut Context,
}

impl<'a> CairoRenderContext<'a> {
    pub fn new(ctx: &mut Context) -> CairoRenderContext {
        CairoRenderContext { ctx }
    }
}

pub enum Brush {
    Solid(u32),
}

pub enum StrokeStyle {
    // TODO: actual stroke style options
    Default,
}

impl<'a> RenderContext for CairoRenderContext<'a> {
    /// Cairo mostly uses raw f64, so this is as convenient as anything.
    type Point = Vec2;
    type Coord = f64;
    type Brush = Brush;
    type StrokeStyle = StrokeStyle;

    fn clear(&mut self, rgb: u32) {
        self.ctx.set_source_rgb(byte_to_frac(rgb >> 16), byte_to_frac(rgb >> 8), byte_to_frac(rgb));
        self.ctx.paint();
    }

    fn solid_brush(&mut self, rgba: u32) -> Brush {
        Brush::Solid(rgba)
    }

    fn line<V: RoundInto<Vec2>, C: RoundInto<f64>>(
        &mut self,
        p0: V,
        p1: V,
        brush: &Self::Brush,
        width: C,
        style: Option<&Self::StrokeStyle>,
    ) {
        self.ctx.new_path();
        let p0 = p0.round_into();
        let p1 = p1.round_into();
        self.ctx.move_to(p0.x, p0.y);
        self.ctx.line_to(p1.x, p1.y);
        self.set_stroke(width.round_into(), style);
        self.set_brush(brush);
        self.ctx.stroke();
    }

    fn fill_path<I: IntoIterator<Item = PathEl>>(&mut self, iter: I, brush: &Self::Brush) {
        self.set_path(iter);
        self.set_brush(brush);
        self.ctx.fill();
    }

    fn stroke_path<I: IntoIterator<Item = PathEl>, C: RoundInto<f64>>(
        &mut self,
        iter: I,
        brush: &Self::Brush,
        width: C,
        style: Option<&Self::StrokeStyle>,
    ) {
        self.set_path(iter);
        self.set_stroke(width.round_into(), style);
        self.set_brush(brush);
        self.ctx.stroke();
    }
}

impl<'a> CairoRenderContext<'a> {
    /// Set the source pattern to the brush.
    ///
    /// Cairo is super stateful, and we're trying to have more retained stuff.
    /// This is part of the impedance matching.
    fn set_brush(&mut self, brush: &Brush) {
        match *brush {
            Brush::Solid(rgba) =>
                self.ctx.set_source_rgba(byte_to_frac(rgba >> 24), byte_to_frac(rgba >> 16),
                    byte_to_frac(rgba >> 8), byte_to_frac(rgba))
        }
    }

    /// Set the stroke parameters.
    fn set_stroke(&mut self, width: f64, style: Option<&StrokeStyle>) {
        self.ctx.set_line_width(width);
        if let Some(style) = style {
            match style {
                // TODO: actual stroke style parameters
                StrokeStyle::Default => (),
            }
        }
    }

    fn set_path<I: IntoIterator<Item = PathEl>>(&mut self, iter: I) {
        // This shouldn't be necessary, we always leave the context in no-path
        // state. But just in case, and it should be harmless.
        self.ctx.new_path();
        let mut last = Vec2::default();
        for el in iter.into_iter() {
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
                    self.ctx.curve_to(c.p1.x, c.p1.y, c.p2.x, c.p2.y, p2.x, p2.y);
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
}

fn byte_to_frac(byte: u32) -> f64 {
    ((byte & 255) as f64) * (1.0 / 255.0)
}
