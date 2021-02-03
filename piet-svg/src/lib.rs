//! SVG output support for piet
//!
//! Text and images are unimplemented and will always return errors.

#![deny(clippy::trivially_copy_pass_by_ref)]

mod text;

use std::borrow::Cow;
use std::{io, mem};

use piet::kurbo::{Affine, Point, Rect, Shape, Size};
use piet::{
    Color, Error, FixedGradient, Image, ImageFormat, InterpolationMode, IntoBrush, LineCap,
    LineJoin, StrokeStyle,
};
use svg::node::Node;

pub use crate::text::{Text, TextLayout};

type Result<T> = std::result::Result<T, Error>;

/// `piet::RenderContext` for generating SVG images
pub struct RenderContext {
    stack: Vec<State>,
    state: State,
    doc: svg::Document,
    next_id: u64,
    text: Text,
}

impl RenderContext {
    /// Construct an empty `RenderContext`
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            state: State::default(),
            doc: svg::Document::new(),
            next_id: 0,
            text: Text::new(),
        }
    }

    /// Write graphics rendered so far to an `std::io::Write` impl, such as `std::fs::File`
    ///
    /// Additional rendering can be done afterwards.
    pub fn write(&self, writer: impl io::Write) -> io::Result<()> {
        svg::write(writer, &self.doc)
    }

    fn new_id(&mut self) -> Id {
        let x = Id(self.next_id);
        self.next_id += 1;
        x
    }
}

impl piet::RenderContext for RenderContext {
    type Brush = Brush;

    type Text = Text;
    type TextLayout = TextLayout;

    type Image = SvgImage;

    fn status(&mut self) -> Result<()> {
        Ok(())
    }

    fn clear(&mut self, color: Color) {
        let mut rect = svg::node::element::Rectangle::new()
            .set("width", "100%")
            .set("height", "100%")
            .set("fill", fmt_color(&color))
            .set("fill-opacity", fmt_opacity(&color));
        if let Some(id) = self.state.clip {
            rect.assign("clip-path", format!("url(#{})", id.to_string()));
        }
        self.doc.append(rect);
    }

    fn solid_brush(&mut self, color: Color) -> Brush {
        Brush {
            kind: BrushKind::Solid(color),
        }
    }

    fn gradient(&mut self, gradient: impl Into<FixedGradient>) -> Result<Brush> {
        let id = self.new_id();
        match gradient.into() {
            FixedGradient::Linear(x) => {
                let mut gradient = svg::node::element::LinearGradient::new()
                    .set("gradientUnits", "userSpaceOnUse")
                    .set("id", id)
                    .set("x1", x.start.x)
                    .set("y1", x.start.y)
                    .set("x2", x.end.x)
                    .set("y2", x.end.y);
                for stop in x.stops {
                    gradient.append(
                        svg::node::element::Stop::new()
                            .set("offset", stop.pos)
                            .set("stop-color", fmt_color(&stop.color))
                            .set("stop-opacity", fmt_opacity(&stop.color)),
                    );
                }
                self.doc.append(gradient);
            }
            FixedGradient::Radial(x) => {
                let mut gradient = svg::node::element::RadialGradient::new()
                    .set("gradientUnits", "userSpaceOnUse")
                    .set("id", id)
                    .set("cx", x.center.x)
                    .set("cy", x.center.y)
                    .set("fx", x.center.x + x.origin_offset.x)
                    .set("fy", x.center.y + x.origin_offset.y)
                    .set("r", x.radius);
                for stop in x.stops {
                    gradient.append(
                        svg::node::element::Stop::new()
                            .set("offset", stop.pos)
                            .set("stop-color", fmt_color(&stop.color))
                            .set("stop-opacity", fmt_opacity(&stop.color)),
                    );
                }
                self.doc.append(gradient);
            }
        }
        Ok(Brush {
            kind: BrushKind::Ref(id),
        })
    }

    fn fill(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        add_shape(
            &mut self.doc,
            shape,
            &Attrs {
                xf: self.state.xf,
                clip: self.state.clip,
                fill: Some((brush.into_owned(), None)),
                ..Attrs::default()
            },
        );
    }

    fn fill_even_odd(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        add_shape(
            &mut self.doc,
            shape,
            &Attrs {
                xf: self.state.xf,
                clip: self.state.clip,
                fill: Some((brush.into_owned(), Some("evenodd"))),
                ..Attrs::default()
            },
        );
    }

    fn clip(&mut self, shape: impl Shape) {
        let id = self.new_id();
        let mut clip = svg::node::element::ClipPath::new().set("id", id);
        add_shape(
            &mut clip,
            shape,
            &Attrs {
                xf: self.state.xf,
                clip: self.state.clip,
                ..Attrs::default()
            },
        );
        self.doc.append(clip);
        self.state.clip = Some(id);
    }

    fn stroke(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>, width: f64) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        add_shape(
            &mut self.doc,
            shape,
            &Attrs {
                xf: self.state.xf,
                clip: self.state.clip,
                stroke: Some((brush.into_owned(), width, &StrokeStyle::new())),
                ..Attrs::default()
            },
        );
    }

    fn stroke_styled(
        &mut self,
        shape: impl Shape,
        brush: &impl IntoBrush<Self>,
        width: f64,
        style: &StrokeStyle,
    ) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        add_shape(
            &mut self.doc,
            shape,
            &Attrs {
                xf: self.state.xf,
                clip: self.state.clip,
                stroke: Some((brush.into_owned(), width, style)),
                ..Attrs::default()
            },
        );
    }

    fn text(&mut self) -> &mut Self::Text {
        &mut self.text
    }

    fn draw_text(&mut self, _layout: &Self::TextLayout, _pos: impl Into<Point>) {
        unimplemented!()
    }

    fn save(&mut self) -> Result<()> {
        let new = self.state.clone();
        self.stack.push(mem::replace(&mut self.state, new));
        Ok(())
    }

    fn restore(&mut self) -> Result<()> {
        self.state = self.stack.pop().ok_or(Error::StackUnbalance)?;
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        Ok(())
    }

    fn transform(&mut self, transform: Affine) {
        self.state.xf *= transform;
    }

    fn current_transform(&self) -> Affine {
        self.state.xf
    }

    fn make_image(
        &mut self,
        _width: usize,
        _height: usize,
        _buf: &[u8],
        _format: ImageFormat,
    ) -> Result<Self::Image> {
        Err(Error::NotSupported)
    }

    #[inline]
    fn draw_image(
        &mut self,
        image: &Self::Image,
        dst_rect: impl Into<Rect>,
        interp: InterpolationMode,
    ) {
        draw_image(self, image, None, dst_rect.into(), interp);
    }

    #[inline]
    fn draw_image_area(
        &mut self,
        image: &Self::Image,
        src_rect: impl Into<Rect>,
        dst_rect: impl Into<Rect>,
        interp: InterpolationMode,
    ) {
        draw_image(self, image, Some(src_rect.into()), dst_rect.into(), interp);
    }

    fn blurred_rect(&mut self, _rect: Rect, _blur_radius: f64, _brush: &impl IntoBrush<Self>) {
        unimplemented!()
    }
}

fn draw_image(
    _ctx: &mut RenderContext,
    _image: &<RenderContext as piet::RenderContext>::Image,
    _src_rect: Option<Rect>,
    _dst_rect: Rect,
    _interp: InterpolationMode,
) {
    unimplemented!()
}

#[derive(Default)]
struct Attrs<'a> {
    xf: Affine,
    clip: Option<Id>,
    fill: Option<(Brush, Option<&'a str>)>,
    stroke: Option<(Brush, f64, &'a StrokeStyle)>,
}

impl Attrs<'_> {
    // allow clippy warning for `width != 1.0` in if statement
    #[allow(clippy::float_cmp)]
    fn apply_to(&self, node: &mut impl Node) {
        node.assign("transform", xf_val(&self.xf));
        if let Some(id) = self.clip {
            node.assign("clip-path", format!("url(#{})", id.to_string()));
        }
        if let Some((ref brush, rule)) = self.fill {
            node.assign("fill", brush.color());
            if let Some(opacity) = brush.opacity() {
                node.assign("fill-opacity", opacity);
            }
            if let Some(rule) = rule {
                node.assign("fill-rule", rule);
            }
        } else {
            node.assign("fill", "none");
        }
        if let Some((ref stroke, width, style)) = self.stroke {
            node.assign("stroke", stroke.color());
            if let Some(opacity) = stroke.opacity() {
                node.assign("stroke-opacity", opacity);
            }
            if width != 1.0 {
                node.assign("stroke-width", width);
            }
            match style.line_join {
                None | Some(LineJoin::Miter) => {}
                Some(LineJoin::Round) => {
                    node.assign("stroke-linejoin", "round");
                }
                Some(LineJoin::Bevel) => {
                    node.assign("stroke-linejoin", "bevel");
                }
            }
            match style.line_cap {
                None | Some(LineCap::Butt) => {}
                Some(LineCap::Round) => {
                    node.assign("stroke-linecap", "round");
                }
                Some(LineCap::Square) => {
                    node.assign("stroke-linecap", "square");
                }
            }
            if let Some((ref array, offset)) = style.dash {
                node.assign("stroke-dasharray", array.clone());
                if offset != 0.0 {
                    node.assign("stroke-dashoffset", offset);
                }
            }
            if let Some(limit) = style.miter_limit {
                node.assign("stroke-miterlimit", limit);
            }
        }
    }
}

fn xf_val(xf: &Affine) -> svg::node::Value {
    let xf = xf.as_coeffs();
    format!(
        "matrix({} {} {} {} {} {})",
        xf[0], xf[1], xf[2], xf[3], xf[4], xf[5]
    )
    .into()
}

fn add_shape(node: &mut impl Node, shape: impl Shape, attrs: &Attrs) {
    if let Some(circle) = shape.as_circle() {
        let mut x = svg::node::element::Circle::new()
            .set("cx", circle.center.x)
            .set("cy", circle.center.y)
            .set("r", circle.radius);
        attrs.apply_to(&mut x);
        node.append(x);
        return;
    } else if let Some(rect) = shape.as_rounded_rect() {
        if let Some(radius) = rect.radii().as_single_radius() {
            let mut x = svg::node::element::Rectangle::new()
                .set("x", rect.origin().x)
                .set("y", rect.origin().y)
                .set("width", rect.width())
                .set("height", rect.height())
                .set("rx", radius)
                .set("ry", radius);
            attrs.apply_to(&mut x);
            node.append(x);
            return;
        }
    } else if let Some(rect) = shape.as_rect() {
        let mut x = svg::node::element::Rectangle::new()
            .set("x", rect.origin().x)
            .set("y", rect.origin().y)
            .set("width", rect.width())
            .set("height", rect.height());
        attrs.apply_to(&mut x);
        node.append(x);
        return;
    }

    let mut path = svg::node::element::Path::new().set("d", shape.into_path(1e-3).to_svg());
    attrs.apply_to(&mut path);
    node.append(path);
}

#[derive(Debug, Clone, Default)]
struct State {
    xf: Affine,
    clip: Option<Id>,
}

/// An SVG brush
#[derive(Debug, Clone)]
pub struct Brush {
    kind: BrushKind,
}

#[derive(Debug, Clone)]
enum BrushKind {
    Solid(Color),
    Ref(Id),
}

impl Brush {
    fn color(&self) -> svg::node::Value {
        match self.kind {
            BrushKind::Solid(ref color) => fmt_color(color).into(),
            BrushKind::Ref(id) => format!("url(#{})", id.to_string()).into(),
        }
    }

    fn opacity(&self) -> Option<svg::node::Value> {
        match self.kind {
            BrushKind::Solid(ref color) => Some(fmt_opacity(color).into()),
            BrushKind::Ref(_) => None,
        }
    }
}

impl IntoBrush<RenderContext> for Brush {
    fn make_brush<'b>(
        &'b self,
        _piet: &mut RenderContext,
        _bbox: impl FnOnce() -> Rect,
    ) -> Cow<'b, Brush> {
        Cow::Owned(self.clone())
    }
}

// RGB in hex representation
fn fmt_color(color: &Color) -> String {
    match color {
        Color::Rgba32(x) => format!("#{:06x}", x >> 8),
    }
}

// Opacity as value from [0, 1]
fn fmt_opacity(color: &Color) -> String {
    match color {
        Color::Rgba32(x) => format!("{}", (x & 0xFF) as f32 / 255.0),
    }
}

/// SVG image (unimplemented)
pub struct SvgImage(());

impl Image for SvgImage {
    fn size(&self) -> Size {
        todo!()
    }
}

#[derive(Debug, Copy, Clone)]
struct Id(u64);

impl Id {
    // TODO allowing clippy warning temporarily. But this should be changed to impl Display
    #[allow(clippy::inherent_to_string)]
    fn to_string(self) -> String {
        const ALPHABET: &[u8; 52] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
        let mut out = String::with_capacity(4);
        let mut x = self.0;
        loop {
            let digit = (x % ALPHABET.len() as u64) as usize;
            out.push(ALPHABET[digit] as char);
            x /= ALPHABET.len() as u64;
            if x == 0 {
                break;
            }
        }
        out
    }
}

impl From<Id> for svg::node::Value {
    fn from(x: Id) -> Self {
        x.to_string().into()
    }
}
