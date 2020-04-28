use ::{
    pathfinder_canvas::{
        CanvasRenderingContext2D, ColorU, FillRule, FillStyle, Path2D, RectF, Vector2F,
    },
    piet::{
        kurbo::{Affine, Circle, Line, PathEl, Point, Rect, Shape},
        Color, Error, FixedGradient, Font, FontBuilder, HitTestPoint, HitTestTextPosition,
        ImageFormat, InterpolationMode, IntoBrush, LineMetric, RenderContext, StrokeStyle, Text,
        TextLayout, TextLayoutBuilder,
    },
    std::{borrow::Cow, f32::consts::PI, ops::Deref},
};

pub struct PfContext<'a> {
    render_ctx: &'a mut CanvasRenderingContext2D,
}

impl<'a> PfContext<'a> {
    pub fn new(render_ctx: &'a mut CanvasRenderingContext2D) -> Self {
        PfContext { render_ctx }
    }
}

impl RenderContext for PfContext<'_> {
    type Brush = FillStyle;
    type Text = PfText;
    type TextLayout = PfTextLayout;
    type Image = ();

    fn status(&mut self) -> Result<(), Error> {
        Err(piet::new_error(piet::ErrorKind::NotSupported))
    }

    fn solid_brush(&mut self, color: Color) -> Self::Brush {
        FillStyle::Color(map_color(color))
    }

    fn gradient(&mut self, gradient: impl Into<FixedGradient>) -> Result<Self::Brush, Error> {
        Err(piet::new_error(piet::ErrorKind::NotSupported))
    }

    fn clear(&mut self, color: Color) {
        let size = self.render_ctx.canvas().size();
        let brush = self.solid_brush(color);
        self.render_ctx.set_fill_style(brush);
        self.render_ctx
            .fill_rect(RectF::new(vec2f(0.0, 0.0), size.to_f32()));
    }

    fn stroke(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>, width: f64) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        self.render_ctx.set_stroke_style(brush.into_owned());
        self.render_ctx.set_line_width(width as f32);
        self.render_ctx.stroke_path(shape_to_path2d(shape));
    }

    fn stroke_styled(
        &mut self,
        shape: impl Shape,
        brush: &impl IntoBrush<Self>,
        width: f64,
        style: &StrokeStyle,
    ) {
        todo!()
    }

    fn fill(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        self.render_ctx.set_fill_style(brush.into_owned());
        self.render_ctx
            .fill_path(shape_to_path2d(shape), FillRule::Winding);
    }

    fn fill_even_odd(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>) {
        todo!()
    }

    fn clip(&mut self, shape: impl Shape) {
        todo!()
    }

    fn text(&mut self) -> &mut Self::Text {
        todo!()
    }

    fn draw_text(
        &mut self,
        layout: &Self::TextLayout,
        pos: impl Into<Point>,
        brush: &impl IntoBrush<Self>,
    ) {
        todo!()
    }

    fn save(&mut self) -> Result<(), Error> {
        Err(piet::new_error(piet::ErrorKind::NotSupported))
    }

    fn restore(&mut self) -> Result<(), Error> {
        Err(piet::new_error(piet::ErrorKind::NotSupported))
    }

    fn finish(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn transform(&mut self, transform: Affine) {
        todo!()
    }

    fn make_image(
        &mut self,
        width: usize,
        height: usize,
        buf: &[u8],
        format: ImageFormat,
    ) -> Result<Self::Image, Error> {
        Err(piet::new_error(piet::ErrorKind::NotSupported))
    }

    fn draw_image(
        &mut self,
        image: &Self::Image,
        dst_rect: impl Into<Rect>,
        interp: InterpolationMode,
    ) {
    }

    fn draw_image_area(
        &mut self,
        image: &Self::Image,
        src_rect: impl Into<Rect>,
        dst_rect: impl Into<Rect>,
        interp: InterpolationMode,
    ) {
    }

    fn blurred_rect(&mut self, rect: Rect, blur_radius: f64, brush: &impl IntoBrush<Self>) {}

    fn current_transform(&self) -> Affine {
        todo!()
    }
}

impl IntoBrush<PfContext<'_>> for FillStyle {
    fn make_brush(
        &self,
        ctx: &mut PfContext,
        bbox: impl FnOnce() -> Rect,
    ) -> Cow<<PfContext as RenderContext>::Brush> {
        Cow::Borrowed(self)
    }
}

#[derive(Clone)]
pub struct PfTextLayout;

impl TextLayout for PfTextLayout {
    fn width(&self) -> f64 {
        todo!()
    }

    fn update_width(&mut self, new_width: impl Into<Option<f64>>) -> Result<(), Error> {
        todo!()
    }

    fn line_text(&self, line_number: usize) -> Option<&str> {
        todo!()
    }

    fn line_metric(&self, line_number: usize) -> Option<LineMetric> {
        todo!()
    }

    fn line_count(&self) -> usize {
        todo!()
    }

    fn hit_test_point(&self, point: Point) -> HitTestPoint {
        todo!()
    }

    fn hit_test_text_position(&self, text_position: usize) -> Option<HitTestTextPosition> {
        todo!()
    }
}

pub struct PfText;

impl Text for PfText {
    type FontBuilder = PfFontBuilder;
    type Font = PfFont;
    type TextLayoutBuilder = PfTextLayoutBuilder;
    type TextLayout = PfTextLayout;
    fn new_font_by_name(&mut self, name: &str, size: f64) -> Self::FontBuilder {
        todo!()
    }
    fn new_text_layout(
        &mut self,
        font: &Self::Font,
        text: &str,
        width: impl Into<Option<f64>>,
    ) -> Self::TextLayoutBuilder {
        todo!()
    }
}

pub struct PfFontBuilder;

impl FontBuilder for PfFontBuilder {
    type Out = PfFont;
    fn build(self) -> Result<Self::Out, Error> {
        todo!()
    }
}

pub struct PfFont;

impl Font for PfFont {}

pub struct PfTextLayoutBuilder;

impl TextLayoutBuilder for PfTextLayoutBuilder {
    type Out = PfTextLayout;
    fn build(self) -> Result<Self::Out, Error> {
        todo!()
    }
}

// helpers

fn map_color(input: Color) -> ColorU {
    let (r, g, b, a) = input.as_rgba_u8();
    ColorU::new(r, g, b, a)
}

fn shape_to_path2d(input: impl Shape) -> Path2D {
    let mut path = Path2D::new();
    if let Some(Line { p0, p1 }) = input.as_line() {
        path.move_to(point_to_vec2f(p0));
        path.line_to(point_to_vec2f(p1));
    } else if let Some(Rect { x0, y0, x1, y1 }) = input.as_rect() {
        path.rect(RectF::new(vec2f(x0, y0), vec2f(x1, y1)));
    } else if let Some(Circle { center, radius }) = input.as_circle() {
        path.ellipse(
            point_to_vec2f(center),
            vec2f(radius, radius),
            0.0,
            0.0,
            2.0 * PI,
        );
    } else if let Some(els) = input.as_path_slice() {
        path_el_iter(&mut path, els.iter().map(|el| *el));
    } else {
        path_el_iter(&mut path, input.to_bez_path(0.1));
    }
    path
}

fn path_el_iter(path: &mut Path2D, iter: impl Iterator<Item = PathEl>) {
    let mut last_move_to: Vector2F = vec2f(0.0, 0.0);
    for el in iter {
        match el {
            PathEl::MoveTo(p) => {
                let p = point_to_vec2f(p);
                last_move_to = p;
                path.move_to(p)
            }
            PathEl::LineTo(p) => path.line_to(point_to_vec2f(p)),
            PathEl::QuadTo(p0, p1) => {
                path.quadratic_curve_to(point_to_vec2f(p0), point_to_vec2f(p1))
            }
            PathEl::CurveTo(p0, p1, p2) => {
                path.bezier_curve_to(point_to_vec2f(p0), point_to_vec2f(p1), point_to_vec2f(p2))
            }
            PathEl::ClosePath => path.line_to(last_move_to),
        }
    }
}

#[inline]
fn vec2f(x: f64, y: f64) -> Vector2F {
    Vector2F::new(x as f32, y as f32)
}

#[inline]
fn point_to_vec2f(p: Point) -> Vector2F {
    vec2f(p.x, p.y)
}
