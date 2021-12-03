//! SVG output support for piet
//!
//! Text and images are unimplemented and will always return errors.

#![deny(clippy::trivially_copy_pass_by_ref)]

mod text;

use std::{borrow::Cow, fmt, io, mem};

use piet::kurbo::{Affine, Point, Rect, Shape, Size};
use piet::{
    Color, Error, FixedGradient, FontStyle, Image, ImageFormat, InterpolationMode, IntoBrush,
    LineCap, LineJoin, StrokeStyle, TextAlignment, TextLayout as _,
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
    pub fn new(size: Option<Size>) -> Self {
        let mut doc = svg::Document::new();
        if let Some(size) = size {
            doc = doc.set("viewBox", (0, 0, size.width, size.height));
        }
        Self {
            stack: Vec::new(),
            state: State::default(),
            doc,
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

    /// Returns an object that can write the svg somewhere.
    pub fn display(&self) -> &impl fmt::Display {
        &self.doc
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

    fn clear(&mut self, rect: impl Into<Option<Rect>>, color: Color) {
        let rect = rect.into();
        let mut rect = match rect {
            Some(rect) => svg::node::element::Rectangle::new()
                .set("width", rect.width())
                .set("height", rect.height())
                .set("x", rect.x0)
                .set("y", rect.y0),
            None => svg::node::element::Rectangle::new()
                .set("width", "100%")
                .set("height", "100%"),
        }
        .set("fill", fmt_color(&color))
        .set("fill-opacity", fmt_opacity(&color));
        //FIXME: I don't think we should be clipping, here?
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

    fn draw_text(&mut self, layout: &Self::TextLayout, pos: impl Into<Point>) {
        let pos = pos.into();

        let color = {
            let (r, g, b, a) = layout.text_color.as_rgba8();
            format!("rgba({}, {}, {}, {})", r, g, b, a as f64 / (255. * 0.01))
        };

        let mut x = pos.x;
        // SVG doesn't do multiline text, and so doesn't have a concept of text width. We can do
        // alignment though, using text-anchor.
        let anchor = match (layout.max_width, layout.alignment) {
            (width, TextAlignment::End) if width.is_finite() && width > 0. => {
                x += width;
                "text-anchor:end"
            }
            (width, TextAlignment::Center) if width.is_finite() && width > 0. => {
                x += width * 0.5;
                "text-anchor:middle"
            }
            _ => "",
        };

        let text = svg::node::element::Text::new()
            .set("x", x)
            .set("y", pos.y)
            .set(
                "style",
                format!(
                    "font-size:{}pt;\
                        font-family:\"{}\";\
                        font-weight:{};\
                        font-style:{};\
                        text-decoration:{};\
                        fill:{};\
                        {}",
                    layout.font_size,
                    layout.font_family.name(),
                    layout.font_weight.to_raw(),
                    match layout.font_style {
                        FontStyle::Regular => "normal",
                        FontStyle::Italic => "italic",
                    },
                    match (layout.underline, layout.strikethrough) {
                        (false, false) => "none",
                        (false, true) => "line-through",
                        (true, false) => "underline",
                        (true, true) => "underline line-through",
                    },
                    color,
                    anchor,
                ),
            )
            .add(svg::node::Text::new(layout.text()));
        self.doc.append(text);
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

    fn capture_image_area(&mut self, _src_rect: impl Into<Rect>) -> Result<Self::Image> {
        Err(Error::Unimplemented)
    }

    fn blurred_rect(&mut self, rect: Rect, _blur_radius: f64, brush: &impl IntoBrush<Self>) {
        // TODO blur (perhaps using SVG filters)
        self.fill(rect, brush)
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
                LineJoin::Miter { limit } if limit == LineJoin::DEFAULT_MITER_LIMIT => (),
                LineJoin::Miter { limit } => {
                    node.assign("stroke-miterlimit", limit);
                }
                LineJoin::Round => {
                    node.assign("stroke-linejoin", "round");
                }
                LineJoin::Bevel => {
                    node.assign("stroke-linejoin", "bevel");
                }
            }
            match style.line_cap {
                LineCap::Round => {
                    node.assign("stroke-linecap", "round");
                }
                LineCap::Square => {
                    node.assign("stroke-linecap", "square");
                }
                LineCap::Butt => (),
            }
            if !style.dash_pattern.is_empty() {
                node.assign("stroke-dasharray", style.dash_pattern.to_vec());
            }
            if style.dash_offset != 0.0 {
                node.assign("stroke-dashoffset", style.dash_offset);
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
    } else if let Some(round_rect) = shape
        .as_rounded_rect()
        .filter(|r| r.radii().as_single_radius().is_some())
    {
        let mut x = svg::node::element::Rectangle::new()
            .set("x", round_rect.origin().x)
            .set("y", round_rect.origin().y)
            .set("width", round_rect.width())
            .set("height", round_rect.height())
            .set("rx", round_rect.radii().as_single_radius().unwrap())
            .set("ry", round_rect.radii().as_single_radius().unwrap());
        attrs.apply_to(&mut x);
        node.append(x);
    } else if let Some(rect) = shape.as_rect() {
        let mut x = svg::node::element::Rectangle::new()
            .set("x", rect.origin().x)
            .set("y", rect.origin().y)
            .set("width", rect.width())
            .set("height", rect.height());
        attrs.apply_to(&mut x);
        node.append(x);
    } else {
        let mut path = svg::node::element::Path::new().set("d", shape.into_path(1e-3).to_svg());
        attrs.apply_to(&mut path);
        node.append(path);
    }
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
    format!("#{:06x}", color.as_rgba_u32() >> 8)
}

// Opacity as value from [0, 1]
fn fmt_opacity(color: &Color) -> String {
    format!("{}", color.as_rgba().3)
}

/// SVG image (unimplemented)
#[derive(Clone)]
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
