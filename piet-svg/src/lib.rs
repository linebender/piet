//! SVG output support for piet
//!
//! Text and images are unimplemented and will always return errors.

use std::borrow::Cow;
use std::{io, mem};

use piet::kurbo::{Affine, Point, Rect, Shape};
use piet::{
    new_error, Color, Error, ErrorKind, FixedGradient, HitTestPoint, HitTestTextPosition,
    ImageFormat, InterpolationMode, IntoBrush, LineCap, LineJoin, StrokeStyle,
};
use svg::node::Node;

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
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            state: State::default(),
            doc: svg::Document::new(),
            next_id: 0,
            text: Text(()),
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

    type Image = Image;

    fn status(&mut self) -> Result<()> {
        Ok(())
    }

    fn clear(&mut self, color: Color) {
        let brush = color.make_brush(self, || Rect::ZERO);
        let mut rect = svg::node::element::Rectangle::new()
            .set("width", "100%")
            .set("height", "100%")
            .set("fill", brush.val());
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
                            .set("stop-color", fmt_color(&stop.color)),
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
                            .set("stop-color", fmt_color(&stop.color)),
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

    fn draw_text(
        &mut self,
        _layout: &Self::TextLayout,
        _pos: impl Into<Point>,
        _brush: &impl IntoBrush<Self>,
    ) {
        unimplemented!()
    }

    fn save(&mut self) -> Result<()> {
        let new = self.state.clone();
        self.stack.push(mem::replace(&mut self.state, new));
        Ok(())
    }

    fn restore(&mut self) -> Result<()> {
        self.state = self
            .stack
            .pop()
            .ok_or_else(|| new_error(ErrorKind::StackUnbalance))?;
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
        Err(new_error(ErrorKind::NotSupported))
    }

    fn draw_image(
        &mut self,
        _image: &Self::Image,
        _rect: impl Into<Rect>,
        _interp: InterpolationMode,
    ) {
        unimplemented!()
    }
}

#[derive(Default)]
struct Attrs<'a> {
    xf: Affine,
    clip: Option<Id>,
    fill: Option<(Brush, Option<&'a str>)>,
    stroke: Option<(Brush, f64, &'a StrokeStyle)>,
}

impl Attrs<'_> {
    fn apply_to(&self, node: &mut impl Node) {
        node.assign("transform", xf_val(&self.xf));
        if let Some(id) = self.clip {
            node.assign("clip-path", format!("url(#{})", id.to_string()));
        }
        if let Some((ref brush, rule)) = self.fill {
            node.assign("fill", brush.val());
            if let Some(rule) = rule {
                node.assign("fill-rule", rule);
            }
        } else {
            node.assign("fill", "none");
        }
        if let Some((ref stroke, width, style)) = self.stroke {
            node.assign("stroke", stroke.val());
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
    } else if let Some(rect) = shape.as_rounded_rect() {
        let mut x = svg::node::element::Rectangle::new()
            .set("x", rect.origin().x)
            .set("y", rect.origin().y)
            .set("width", rect.width())
            .set("height", rect.height())
            .set("rx", rect.radius())
            .set("ry", rect.radius());
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
        let mut path = svg::node::element::Path::new().set("d", shape.into_bez_path(1e-3).to_svg());
        attrs.apply_to(&mut path);
        node.append(path)
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
    fn val(&self) -> svg::node::Value {
        match self.kind {
            BrushKind::Solid(ref color) => fmt_color(color).into(),
            BrushKind::Ref(id) => format!("url(#{})", id.to_string()).into(),
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

fn fmt_color(color: &Color) -> String {
    match color {
        Color::Rgba32(x) => format!("#{:08x}", x),
    }
}

/// SVG text (unimplemented)
pub struct Text(());

impl piet::Text for Text {
    type Font = Font;
    type FontBuilder = FontBuilder;
    type TextLayout = TextLayout;
    type TextLayoutBuilder = TextLayoutBuilder;

    fn new_font_by_name(&mut self, _name: &str, _size: f64) -> FontBuilder {
        FontBuilder(())
    }

    fn new_text_layout(&mut self, _font: &Self::Font, _text: &str) -> TextLayoutBuilder {
        TextLayoutBuilder(())
    }
}

/// SVG font builder (unimplemented)
pub struct FontBuilder(());

impl piet::FontBuilder for FontBuilder {
    type Out = Font;

    fn build(self) -> Result<Font> {
        Err(new_error(ErrorKind::NotSupported))
    }
}

/// SVG font (unimplemented)
pub struct Font(());

impl piet::Font for Font {}

pub struct TextLayoutBuilder(());

impl piet::TextLayoutBuilder for TextLayoutBuilder {
    type Out = TextLayout;

    fn build(self) -> Result<TextLayout> {
        Err(new_error(ErrorKind::NotSupported))
    }
}

/// SVG text layout (unimplemented)
pub struct TextLayout(());

impl piet::TextLayout for TextLayout {
    fn width(&self) -> f64 {
        unimplemented!()
    }

    fn hit_test_point(&self, _point: Point) -> HitTestPoint {
        unimplemented!()
    }

    fn hit_test_text_position(&self, _text_position: usize) -> Option<HitTestTextPosition> {
        unimplemented!()
    }
}

/// SVG image (unimplemented)
pub struct Image(());

#[derive(Debug, Copy, Clone)]
struct Id(u64);

impl Id {
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
