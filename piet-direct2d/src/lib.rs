#![cfg(windows)]
//! The Direct2D backend for the Piet 2D graphics abstraction.

mod conv;
pub mod error;

use crate::conv::{
    affine_to_matrix3x2f, color_to_colorf, convert_stroke_style, gradient_stop_to_d2d,
    rect_to_rectf, to_point2f,
};
use crate::error::WrapError;

use std::borrow::Cow;
use std::convert::TryInto;

use winapi::shared::basetsd::UINT32;
use winapi::um::dcommon::D2D_SIZE_U;

use dxgi::Format;

use direct2d::brush::gradient::linear::LinearGradientBrushBuilder;
use direct2d::brush::gradient::radial::RadialGradientBrushBuilder;
pub use direct2d::brush::GenericBrush;
use direct2d::brush::{Brush, SolidColorBrush};
use direct2d::enums::{
    AlphaMode, BitmapInterpolationMode, DrawTextOptions, FigureBegin, FigureEnd, FillMode,
};
use direct2d::geometry::path::{FigureBuilder, GeometryBuilder};
use direct2d::geometry::Path;
use direct2d::image::Bitmap;
use direct2d::layer::Layer;
use direct2d::math::{BezierSegment, QuadBezierSegment, SizeU, Vector2F};
use direct2d::render_target::{GenericRenderTarget, RenderTarget};

use directwrite::text_format::TextFormatBuilder;
use directwrite::text_layout;
use directwrite::TextFormat;

use piet::kurbo::{Affine, PathEl, Point, Rect, Shape};

use piet::{
    new_error, Color, Error, ErrorKind, FixedGradient, Font, FontBuilder, ImageFormat,
    InterpolationMode, IntoBrush, RenderContext, StrokeStyle, Text, TextLayout, TextLayoutBuilder, HitTestPoint, HitTestTextPosition, HitTestMetrics,
};

pub struct D2DRenderContext<'a> {
    factory: &'a direct2d::Factory,
    inner_text: D2DText<'a>,
    // This is an owned clone, but after some direct2d refactor, it's likely we'll
    // hold a mutable reference.
    rt: GenericRenderTarget,

    /// The context state stack. There is always at least one, until finishing.
    ctx_stack: Vec<CtxState>,

    err: Result<(), Error>,
}

pub struct D2DText<'a> {
    dwrite: &'a directwrite::Factory,
}

pub struct D2DFont(TextFormat);

pub struct D2DFontBuilder<'a> {
    builder: TextFormatBuilder<'a>,
    name: String,
}

pub struct D2DTextLayout {
    text: String,
    layout: text_layout::TextLayout,
}

pub struct D2DTextLayoutBuilder<'a> {
    builder: text_layout::TextLayoutBuilder<'a>,
    format: TextFormat,
    text: String,
}

#[derive(Default)]
struct CtxState {
    transform: Affine,

    // Note: when we start pushing both layers and axis aligned clips, this will
    // need to keep track of which is which. But for now, keep it simple.
    n_layers_pop: usize,
}

impl<'b, 'a: 'b> D2DRenderContext<'a> {
    /// Create a new Piet RenderContext for the Direct2D RenderTarget.
    ///
    /// Note: the signature of this function has more restrictive lifetimes than
    /// the implementation requires, because we actually clone the RT, but this
    /// will likely change.
    pub fn new<RT: RenderTarget>(
        factory: &'a direct2d::Factory,
        dwrite: &'a directwrite::Factory,
        rt: &'b mut RT,
    ) -> D2DRenderContext<'b> {
        let inner_text = D2DText { dwrite };
        D2DRenderContext {
            factory,
            inner_text: inner_text,
            rt: rt.as_generic(),
            ctx_stack: vec![CtxState::default()],
            err: Ok(()),
        }
    }

    fn current_transform(&self) -> Affine {
        // This is an unwrap because we protect the invariant.
        self.ctx_stack.last().unwrap().transform
    }

    fn pop_state(&mut self) {
        // This is an unwrap because we protect the invariant.
        let old_state = self.ctx_stack.pop().unwrap();
        for _ in 0..old_state.n_layers_pop {
            self.rt.pop_layer();
        }
    }
}

enum PathBuilder<'a> {
    Geom(GeometryBuilder<'a>),
    Fig(FigureBuilder<'a>),
}

impl<'a> PathBuilder<'a> {
    fn finish_figure(self) -> GeometryBuilder<'a> {
        match self {
            PathBuilder::Geom(g) => g,
            PathBuilder::Fig(f) => f.end(),
        }
    }
}

// The setting of 1e-3 is extremely conservative (absolutely no
// differences should be visible) but setting a looser tolerance is
// likely a tiny performance improvement. We could fine-tune based on
// empirical study of both quality and performance.
const BEZ_TOLERANCE: f64 = 1e-3;

fn path_from_shape(
    d2d: &direct2d::Factory,
    is_filled: bool,
    shape: impl Shape,
    fill_mode: FillMode,
) -> Result<Path, Error> {
    let mut path = Path::create(d2d).wrap()?;
    {
        let mut g = path.open().wrap()?;
        if fill_mode == FillMode::Winding {
            g = g.fill_mode(fill_mode);
        }
        let mut builder = Some(PathBuilder::Geom(g));
        // Note: this is the allocate + clone version so we can scan forward
        // to determine whether the path is closed. Switch to non-allocating
        // version when updating the direct2d bindings.
        let bez_path = shape.into_bez_path(BEZ_TOLERANCE);
        let bez_elements = bez_path.elements();
        for i in 0..bez_elements.len() {
            let el = &bez_elements[i];
            match el {
                PathEl::MoveTo(p) => {
                    let mut is_closed = is_filled;
                    // Scan forward to see if we need to close the path. The
                    // need for this will go away when we udpate direct2d.
                    if !is_filled {
                        for close_el in &bez_elements[i + 1..] {
                            match close_el {
                                PathEl::MoveTo(_) => break,
                                PathEl::ClosePath => {
                                    is_closed = true;
                                    break;
                                }
                                _ => (),
                            }
                        }
                    }
                    if let Some(b) = builder.take() {
                        let g = b.finish_figure();
                        let begin = if is_filled {
                            FigureBegin::Filled
                        } else {
                            FigureBegin::Hollow
                        };
                        let end = if is_closed {
                            FigureEnd::Closed
                        } else {
                            FigureEnd::Open
                        };
                        let f = g.begin_figure(to_point2f(*p), begin, end);
                        builder = Some(PathBuilder::Fig(f));
                    }
                }
                PathEl::LineTo(p) => {
                    if let Some(PathBuilder::Fig(f)) = builder.take() {
                        let f = f.add_line(to_point2f(*p));
                        builder = Some(PathBuilder::Fig(f));
                    }
                }
                PathEl::QuadTo(p1, p2) => {
                    if let Some(PathBuilder::Fig(f)) = builder.take() {
                        let q = QuadBezierSegment::new(to_point2f(*p1), to_point2f(*p2));
                        let f = f.add_quadratic_bezier(&q);
                        builder = Some(PathBuilder::Fig(f));
                    }
                }
                PathEl::CurveTo(p1, p2, p3) => {
                    if let Some(PathBuilder::Fig(f)) = builder.take() {
                        let c =
                            BezierSegment::new(to_point2f(*p1), to_point2f(*p2), to_point2f(*p3));
                        let f = f.add_bezier(&c);
                        builder = Some(PathBuilder::Fig(f));
                    }
                }
                _ => (),
            }
        }
        if let Some(b) = builder.take() {
            // TODO: think about what to do on error
            let _ = b.finish_figure().close();
        }
    }
    Ok(path)
}

impl<'a> RenderContext for D2DRenderContext<'a> {
    type Brush = GenericBrush;

    type Text = D2DText<'a>;

    type TextLayout = D2DTextLayout;

    type Image = Bitmap;

    fn status(&mut self) -> Result<(), Error> {
        std::mem::replace(&mut self.err, Ok(()))
    }

    fn clear(&mut self, color: Color) {
        self.rt.clear(color.as_rgba_u32() >> 8);
    }

    fn solid_brush(&mut self, color: Color) -> GenericBrush {
        SolidColorBrush::create(&self.rt)
            .with_color(color_to_colorf(color))
            .build()
            .wrap()
            .expect("error creating solid brush")
            .to_generic() // This does an extra COM clone; avoid somehow?
    }

    fn gradient(&mut self, gradient: impl Into<FixedGradient>) -> Result<GenericBrush, Error> {
        match gradient.into() {
            FixedGradient::Linear(linear) => {
                let mut builder = LinearGradientBrushBuilder::new(&self.rt)
                    .with_start(to_point2f(linear.start))
                    .with_end(to_point2f(linear.end));
                for stop in &linear.stops {
                    builder = builder.with_stop(gradient_stop_to_d2d(stop));
                }
                let brush = builder.build().wrap()?;
                // Same concern about extra COM clone as above.
                Ok(brush.to_generic())
            }
            FixedGradient::Radial(radial) => {
                let radius = radial.radius as f32;
                let mut builder = RadialGradientBrushBuilder::new(&self.rt)
                    .with_center(to_point2f(radial.center))
                    .with_origin_offset(to_point2f(radial.origin_offset))
                    .with_radius(radius, radius);
                for stop in &radial.stops {
                    builder = builder.with_stop(gradient_stop_to_d2d(stop));
                }
                let brush = builder.build().wrap()?;
                // Ditto
                Ok(brush.to_generic())
            }
        }
    }

    fn fill(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>) {
        // TODO: various special-case shapes, for efficiency
        let brush = brush.make_brush(self, || shape.bounding_box());
        match path_from_shape(self.factory, true, shape, FillMode::Winding) {
            Ok(path) => self.rt.fill_geometry(&path, &*brush),
            Err(e) => self.err = Err(e),
        }
    }

    fn fill_even_odd(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>) {
        // TODO: various special-case shapes, for efficiency
        let brush = brush.make_brush(self, || shape.bounding_box());
        match path_from_shape(self.factory, true, shape, FillMode::Alternate) {
            Ok(path) => self.rt.fill_geometry(&path, &*brush),
            Err(e) => self.err = Err(e),
        }
    }

    fn stroke(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>, width: f64) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        // TODO: various special-case shapes, for efficiency
        let path = match path_from_shape(self.factory, false, shape, FillMode::Alternate) {
            Ok(path) => path,
            Err(e) => {
                self.err = Err(e);
                return;
            }
        };
        let width = width as f32;
        self.rt.draw_geometry(&path, &*brush, width, None);
    }

    fn stroke_styled(
        &mut self,
        shape: impl Shape,
        brush: &impl IntoBrush<Self>,
        width: f64,
        style: &StrokeStyle,
    ) {
        let brush = brush.make_brush(self, || shape.bounding_box());
        // TODO: various special-case shapes, for efficiency
        let path = match path_from_shape(self.factory, false, shape, FillMode::Alternate) {
            Ok(path) => path,
            Err(e) => {
                self.err = Err(e);
                return;
            }
        };
        let width = width as f32;
        let style = convert_stroke_style(self.factory, style, width)
            .expect("stroke style conversion failed");
        self.rt.draw_geometry(&path, &*brush, width, Some(&style));
    }

    fn clip(&mut self, shape: impl Shape) {
        // TODO: set size based on bbox of shape.
        let layer = match Layer::create(&mut self.rt, None).wrap() {
            Ok(layer) => layer,
            Err(e) => {
                self.err = Err(e);
                return;
            }
        };
        let path = match path_from_shape(self.factory, true, shape, FillMode::Winding) {
            Ok(path) => path,
            Err(e) => {
                self.err = Err(e);
                return;
            }
        };
        // TODO: we get a use-after-free crash if we don't do this. Almost certainly
        // this will be fixed in direct2d 0.3, so remove workaround when upgrading.
        let _clone = path.clone();
        self.rt.push_layer(&layer).with_mask(path).push();
        self.ctx_stack.last_mut().unwrap().n_layers_pop += 1;
    }

    fn text(&mut self) -> &mut Self::Text {
        &mut self.inner_text
    }

    fn draw_text(
        &mut self,
        layout: &Self::TextLayout,
        pos: impl Into<Point>,
        brush: &impl IntoBrush<Self>,
    ) {
        // TODO: bounding box for text
        let brush = brush.make_brush(self, || Rect::ZERO);
        let mut line_metrics = Vec::with_capacity(1);
        layout.layout.get_line_metrics(&mut line_metrics);
        if line_metrics.is_empty() {
            // Layout is empty, don't bother drawing.
            return;
        }
        // Direct2D takes upper-left, so adjust for baseline.
        let pos = to_point2f(pos.into());
        let pos = pos - Vector2F::new(0.0, line_metrics[0].baseline());
        // TODO: set ENABLE_COLOR_FONT on Windows 8.1 and above, need version sniffing.
        let text_options = DrawTextOptions::NONE;

        self.rt
            .draw_text_layout(pos, &layout.layout, &*brush, text_options);
    }

    fn save(&mut self) -> Result<(), Error> {
        let new_state = CtxState {
            transform: self.current_transform(),
            n_layers_pop: 0,
        };
        self.ctx_stack.push(new_state);
        Ok(())
    }

    fn restore(&mut self) -> Result<(), Error> {
        if self.ctx_stack.len() <= 1 {
            return Err(new_error(ErrorKind::StackUnbalance));
        }
        self.pop_state();
        // Move this code into impl to avoid duplication with transform?
        self.rt
            .set_transform(&affine_to_matrix3x2f(self.current_transform()));
        Ok(())
    }

    // Discussion question: should this subsume EndDraw, with BeginDraw on
    // D2DRenderContext creation? I'm thinking not, as the shell might want
    // to do other stuff, possibly related to incremental paint.
    fn finish(&mut self) -> Result<(), Error> {
        if self.ctx_stack.len() != 1 {
            return Err(new_error(ErrorKind::StackUnbalance));
        }
        self.pop_state();
        std::mem::replace(&mut self.err, Ok(()))
    }

    fn transform(&mut self, transform: Affine) {
        self.ctx_stack.last_mut().unwrap().transform *= transform;
        self.rt
            .set_transform(&affine_to_matrix3x2f(self.current_transform()));
    }

    fn make_image(
        &mut self,
        width: usize,
        height: usize,
        buf: &[u8],
        format: ImageFormat,
    ) -> Result<Self::Image, Error> {
        // TODO: this method _really_ needs error checking, so much can go wrong...
        let alpha_mode = match format {
            ImageFormat::Rgb => AlphaMode::Ignore,
            ImageFormat::RgbaPremul | ImageFormat::RgbaSeparate => AlphaMode::Premultiplied,
            _ => return Err(new_error(ErrorKind::NotSupported)),
        };
        let buf = match format {
            ImageFormat::Rgb => {
                let mut new_buf = vec![255; width * height * 4];
                for i in 0..width * height {
                    new_buf[i * 4 + 0] = buf[i * 3 + 0];
                    new_buf[i * 4 + 1] = buf[i * 3 + 1];
                    new_buf[i * 4 + 2] = buf[i * 3 + 2];
                }
                Cow::from(new_buf)
            }
            ImageFormat::RgbaSeparate => {
                let mut new_buf = vec![255; width * height * 4];
                // TODO (performance): this would be soooo much faster with SIMD
                fn premul(x: u8, a: u8) -> u8 {
                    let y = (x as u16) * (a as u16);
                    ((y + (y >> 8) + 0x80) >> 8) as u8
                }
                for i in 0..width * height {
                    let a = buf[i * 4 + 3];
                    new_buf[i * 4 + 0] = premul(buf[i * 4 + 0], a);
                    new_buf[i * 4 + 1] = premul(buf[i * 4 + 1], a);
                    new_buf[i * 4 + 2] = premul(buf[i * 4 + 2], a);
                    new_buf[i * 4 + 3] = a;
                }
                Cow::from(new_buf)
            }
            ImageFormat::RgbaPremul => Cow::from(buf),
            // This should be unreachable, we caught it above.
            _ => return Err(new_error(ErrorKind::NotSupported)),
        };
        Bitmap::create(&self.rt)
            .with_raw_data(
                SizeU(D2D_SIZE_U {
                    width: width as UINT32,
                    height: height as UINT32,
                }),
                &buf,
                width as UINT32 * 4,
            )
            .with_format(Format::R8G8B8A8Unorm)
            .with_alpha_mode(alpha_mode)
            .build()
            .wrap()
    }

    fn draw_image(
        &mut self,
        image: &Self::Image,
        rect: impl Into<Rect>,
        interp: InterpolationMode,
    ) {
        let interp = match interp {
            InterpolationMode::NearestNeighbor => BitmapInterpolationMode::NearestNeighbor,
            InterpolationMode::Bilinear => BitmapInterpolationMode::Linear,
        };
        let src_size = image.get_size();
        let src_rect = (0.0, 0.0, src_size.0.width, src_size.0.height);
        self.rt
            .draw_bitmap(&image, rect_to_rectf(rect.into()), 1.0, interp, src_rect);
    }
}

impl<'a> IntoBrush<D2DRenderContext<'a>> for GenericBrush {
    fn make_brush<'b>(
        &'b self,
        _piet: &mut D2DRenderContext,
        _bbox: impl FnOnce() -> Rect,
    ) -> std::borrow::Cow<'b, GenericBrush> {
        Cow::Borrowed(self)
    }
}

impl<'a> D2DText<'a> {
    /// Create a new factory that satisfies the piet `Text` trait given
    /// the (platform-specific) dwrite factory.
    pub fn new(dwrite: &'a directwrite::Factory) -> D2DText<'a> {
        D2DText { dwrite }
    }
}

impl<'a> Text for D2DText<'a> {
    type FontBuilder = D2DFontBuilder<'a>;
    type Font = D2DFont;
    type TextLayoutBuilder = D2DTextLayoutBuilder<'a>;
    type TextLayout = D2DTextLayout;

    fn new_font_by_name(&mut self, name: &str, size: f64) -> Self::FontBuilder {
        // Note: the name is cloned here, rather than applied using `with_family` for
        // lifetime reasons. Maybe there's a better approach.
        D2DFontBuilder {
            builder: TextFormat::create(self.dwrite).with_size(size as f32),
            name: name.to_owned(),
        }
    }

    fn new_text_layout(&mut self, font: &Self::Font, text: &str) -> Self::TextLayoutBuilder {
        // Same consideration as above, we clone the font and text for lifetime
        // reasons.
        D2DTextLayoutBuilder {
            builder: text_layout::TextLayout::create(self.dwrite),
            format: font.0.clone(),
            text: text.to_owned(),
        }
    }
}

impl<'a> FontBuilder for D2DFontBuilder<'a> {
    type Out = D2DFont;

    fn build(self) -> Result<Self::Out, Error> {
        Ok(D2DFont(
            self.builder.with_family(&self.name).build().wrap()?,
        ))
    }
}

impl Font for D2DFont {}

impl<'a> TextLayoutBuilder for D2DTextLayoutBuilder<'a> {
    type Out = D2DTextLayout;

    fn build(self) -> Result<Self::Out, Error> {
        Ok(D2DTextLayout {
            layout: self.builder
                    .with_text(&self.text)
                    .with_font(&self.format)
                    .with_width(1e6) // TODO: probably want to support wrapping
                    .with_height(1e6)
                    .build()
                    .wrap()?,
            text: self.text,
        })
    }
}

impl TextLayout for D2DTextLayout {
    fn width(&self) -> f64 {
        self.layout.get_metrics().width() as f64
    }

    /// Hit Test for Point
    ///
    /// Given a Point, returns the text position which corresponds to the nearest leading grapheme
    /// cluster boundary.
    ///
    /// `text.len()` is a valid position; it's the last valid "cursor position" at the end of the
    /// line.
    fn hit_test_point(&self, point: Point) -> HitTestPoint {
        // lossy from f64 to f32, but shouldn't have too much impact
        let htp = self.layout.hit_test_point(
            point.x as f32,
            point.y as f32,
        );

        // Round up to next grapheme cluster boundary if directwrite
        // reports a trailing hit.
        let text_position_16 = if htp.is_trailing_hit {
            htp.metrics.text_position() + htp.metrics.length()
        } else {
            htp.metrics.text_position()
        } as usize;


        // Convert text position from utf-16 code units to
        // utf-8 code units.
        // Strategy: count up in utf16 and utf8 simultaneously, stop when
        // utf-16 text position reached.
        //
        // TODO ask about text_position, it looks like windows returns last index;
        // can't use the text_position of last index from directwrite, it has an extra code unit.
        let text_position = count_until_utf16(&self.text, text_position_16)
            .unwrap_or(self.text.len());


        HitTestPoint {
            metrics: HitTestMetrics {
                text_position,
                is_text: htp.metrics.is_text(),
            },
            is_inside: htp.is_inside,
            is_trailing_hit: false, // not doing BIDI for now, so will never use trailing
        }
    }

    /// Hit Test for Text Position.
    ///
    /// Given a text position (as a utf-8 code unit), returns the `x` offset of the associated grapheme cluster (generally).
    /// Setting `trailing` to `true` will give the trailing offset, otherwise the leading offset.
    /// Can panic if text position is not at a code point boundary, or if it's out of bounds.
    fn hit_test_text_position(&self, text_position: usize, trailing: bool) -> Option<HitTestTextPosition> {
        // Note: Directwrite will just return the line width if text position is
        // out of bounds. This is what want for piet; return line width for the last text position
        // (equal to line.len()). This is basically returning line width for the last cursor
        // position.

        // Now convert the utf8 index to utf16.
        // This can panic;
        let idx_16 = count_utf16(&self.text[0..text_position]);

        // panic or Result are also fine options for dealing with overflow. Using Option here
        // because it's already present and convenient.
        // TODO this should probably go before convertin to utf16, since that's relatively slow
        let idx_16 = idx_16.try_into().ok()?;

        // TODO quick fix until directwrite fixes bool bug
        let trailing = !trailing;

        self.layout.hit_test_text_position(idx_16, trailing)
            .map(|http| {
                HitTestTextPosition {
                    point: Point {
                        x: http.point_x as f64,
                        y: http.point_y as f64,
                    },
                    metrics: HitTestMetrics {
                        text_position: text_position, // no need to use directwrite return value
                        is_text: http.metrics.is_text(),
                    },
                }
            })
    }
}

/// Counts the number of utf-16 code units in the given string.
/// from xi-editor
pub(crate) fn count_utf16(s: &str) -> usize {
    let mut utf16_count = 0;
    for &b in s.as_bytes() {
        if (b as i8) >= -0x40 {
            utf16_count += 1;
        }
        if b >= 0xf0 {
            utf16_count += 1;
        }
    }
    utf16_count
}

/// returns utf8 text position (code unit offset)
/// at the given utf-16 text position
pub(crate) fn count_until_utf16(s: &str, utf16_text_position: usize) -> Option<usize> {
    let mut utf8_count = 0;
    let mut utf16_count = 0;
    for &b in s.as_bytes() {
        if (b as i8) >= -0x40 {
            utf16_count += 1;
        }
        if b >= 0xf0 {
            utf16_count += 1;
        }

        // When count goes beyond text position, it means the start boundary of the utf16 code unit is passed.
        // So the utf8 count needs to be backtracked 1.
        //
        // char  | utf8 | utf16 | 16_count
        // é     | 0    | 0     | 0
        //       | 1    | -     | 1
        // {0023}| 2    | 1     | 1
        // {FE0F}| 3    | 2     | 2
        //       | 4    | -     | 3
        //       | 5    | -     | 3
        // {20E3}| 6    | 3     | 3
        //       | 7    | -     | 4
        //       | 8    | -     | 4
        // 1     | 9    | -     | 4
        if utf16_count > utf16_text_position {
            return Some(utf8_count);
        }

        utf8_count += 1;
    }

    None
}

#[cfg(test)]
mod test {
    use crate::*;
    use piet::TextLayout;

    #[test]
    fn test_hit_test_text_position_basic() {
        let dwrite = directwrite::factory::Factory::new().unwrap();
        let mut text_layout = D2DText::new(&dwrite);
        let font = text_layout.new_font_by_name("Segoe UI", 12.0).build().unwrap();

        let layout = text_layout.new_text_layout(&font, "piet").build().unwrap();
        let piet_width = layout.width();

        let layout = text_layout.new_text_layout(&font, "pie").build().unwrap();
        let pie_width = layout.width();

        let layout = text_layout.new_text_layout(&font, "pi").build().unwrap();
        let pi_width = layout.width();

        let layout = text_layout.new_text_layout(&font, "p").build().unwrap();
        let p_width = layout.width();

        let layout = text_layout.new_text_layout(&font, "").build().unwrap();
        let null_width = layout.width();

        let full_layout = text_layout.new_text_layout(&font, "piet text!").build().unwrap();

        assert_eq!(full_layout.hit_test_text_position(3, true).map(|p| p.point.x as f64), Some(piet_width));
        assert_eq!(full_layout.hit_test_text_position(2, true).map(|p| p.point.x as f64), Some(pie_width));
        assert_eq!(full_layout.hit_test_text_position(1, true).map(|p| p.point.x as f64), Some(pi_width));
        assert_eq!(full_layout.hit_test_text_position(0, true).map(|p| p.point.x as f64), Some(p_width));

        assert_eq!(full_layout.hit_test_text_position(0, false).map(|p| p.point.x as f64), Some(null_width));
        assert_eq!(full_layout.hit_test_text_position(9, true).map(|p| p.point.x as f64), Some(full_layout.width()));
        assert_eq!(full_layout.hit_test_text_position(10, false).map(|p| p.point.x as f64), Some(full_layout.width()));
    }

    #[test]
    fn test_hit_test_text_position_complex_0() {
        let dwrite = directwrite::factory::Factory::new().unwrap();

        let input = "é";

        let mut text_layout = D2DText::new(&dwrite);
        let font = text_layout.new_font_by_name("Segoe UI", 12.0).build().unwrap();
        let layout = text_layout.new_text_layout(&font, input).build().unwrap();

        assert_eq!(layout.hit_test_text_position(0, true).map(|p| p.point.x), Some(layout.width()));
        assert_eq!(input.len(), 2);

        // unicode segmentation is wrong on this one for now.
        //let input = "🤦\u{1f3fc}\u{200d}\u{2642}\u{fe0f}";

        //let mut text_layout = D2DText::new();
        //let font = text_layout.new_font_by_name("Segoe UI", 12.0).build().unwrap();
        //let layout = text_layout.new_text_layout(&font, input).build().unwrap();

        //assert_eq!(input.graphemes(true).count(), 1);
        //assert_eq!(layout.hit_test_text_position(0, true).map(|p| p.point_x as f64), Some(layout.width()));
        //assert_eq!(input.len(), 17);

        let input = "\u{0023}\u{FE0F}\u{20E3}"; // #️⃣

        let mut text_layout = D2DText::new(&dwrite);
        let font = text_layout.new_font_by_name("Segoe UI", 12.0).build().unwrap();
        let layout = text_layout.new_text_layout(&font, input).build().unwrap();

        assert_eq!(layout.hit_test_text_position(0, true).map(|p| p.point.x), Some(layout.width()));
        assert_eq!(input.len(), 7);
        assert_eq!(input.chars().count(), 3);
    }

    #[test]
    fn test_hit_test_text_position_complex_1() {
        let dwrite = directwrite::factory::Factory::new().unwrap();

        // Notes on this input:
        // 5 code points
        // 5 utf-16 code units
        // 10 utf-8 code units (2/1/3/3/1)
        // 3 graphemes
        let input = "é\u{0023}\u{FE0F}\u{20E3}1"; // #️⃣

        let mut text_layout = D2DText::new(&dwrite);
        let font = text_layout.new_font_by_name("Segoe UI", 12.0).build().unwrap();
        let layout = text_layout.new_text_layout(&font, input).build().unwrap();

        let test_layout_0 = text_layout.new_text_layout(&font, "é").build().unwrap();
        let test_layout_1 = text_layout.new_text_layout(&font, "é\u{0023}\u{FE0F}\u{20E3}").build().unwrap();

        //assert_eq!(input.graphemes(true).count(), 3);
        assert_eq!(input.len(), 10);

        // Note: text position is in terms of utf8 code units
        assert_eq!(layout.hit_test_text_position(0, true).map(|p| p.point.x), Some(test_layout_0.width()));
        assert_eq!(layout.hit_test_text_position(2, true).map(|p| p.point.x), Some(test_layout_1.width()));
        assert_eq!(layout.hit_test_text_position(9, true).map(|p| p.point.x), Some(layout.width()));

        assert_eq!(layout.hit_test_text_position(2, false).map(|p| p.point.x), Some(test_layout_0.width()));
        assert_eq!(layout.hit_test_text_position(9, false).map(|p| p.point.x), Some(test_layout_1.width()));
    }

    #[test]
    fn test_hit_test_point_basic() {
        let dwrite = directwrite::factory::Factory::new().unwrap();

        let mut text_layout = D2DText::new(&dwrite);

        let font = text_layout.new_font_by_name("Segoe UI", 12.0).build().unwrap();
        let layout = text_layout.new_text_layout(&font, "piet text!").build().unwrap();
        println!("text pos 4 leading: {:?}", layout.hit_test_text_position(4, false)); // 20.302734375
        println!("text pos 4 trailing: {:?}", layout.hit_test_text_position(5, false)); // 23.58984375

        // test hit test point
        // all inside
        let pt = layout.hit_test_point(Point::new(21.0, 0.0));
        assert_eq!(pt.metrics.text_position, 4);
        assert_eq!(pt.is_trailing_hit, false);
        let pt = layout.hit_test_point(Point::new(22.0, 0.0));
        assert_eq!(pt.metrics.text_position, 5);
        assert_eq!(pt.is_trailing_hit, false);
        let pt = layout.hit_test_point(Point::new(23.0, 0.0));
        assert_eq!(pt.metrics.text_position, 5);
        assert_eq!(pt.is_trailing_hit, false);
        let pt = layout.hit_test_point(Point::new(24.0, 0.0));
        assert_eq!(pt.metrics.text_position, 5);
        assert_eq!(pt.is_trailing_hit, false);
        let pt = layout.hit_test_point(Point::new(25.0, 0.0));
        assert_eq!(pt.metrics.text_position, 5);
        assert_eq!(pt.is_trailing_hit, false);

        // outside
        println!("layout_width: {:?}", layout.width()); // 46.916015625

        let pt = layout.hit_test_point(Point::new(48.0, 0.0));
        assert_eq!(pt.metrics.text_position, 10); // last text position
        assert_eq!(pt.is_trailing_hit, false);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(-1.0, 0.0));
        assert_eq!(pt.metrics.text_position, 0); // first text position
        assert_eq!(pt.is_trailing_hit, false);
        assert_eq!(pt.is_inside, false);
    }

    #[test]
    fn test_hit_test_point_complex() {
        let dwrite = directwrite::factory::Factory::new().unwrap();

        // Notes on this input:
        // 5 code points
        // 5 utf-16 code units
        // 10 utf-8 code units (2/1/3/3/1)
        // 3 graphemes
        let input = "é\u{0023}\u{FE0F}\u{20E3}1"; // #️⃣

        let mut text_layout = D2DText::new(&dwrite);
        let font = text_layout.new_font_by_name("Segoe UI", 12.0).build().unwrap();
        let layout = text_layout.new_text_layout(&font, input).build().unwrap();
        println!("text pos 2 leading: {:?}", layout.hit_test_text_position(2, false)); // 6.275390625
        println!("text pos 2 trailing: {:?}", layout.hit_test_text_position(9, false)); // 18.0
        println!("text pos 2 trailing: {:?}", layout.hit_test_text_position(10, false)); // 24.46875, line width

        let pt = layout.hit_test_point(Point::new(2.0, 0.0));
        assert_eq!(pt.metrics.text_position, 0);
        assert_eq!(pt.is_trailing_hit, false);
        let pt = layout.hit_test_point(Point::new(4.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
        assert_eq!(pt.is_trailing_hit, false);
        let pt = layout.hit_test_point(Point::new(7.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
        assert_eq!(pt.is_trailing_hit, false);
        let pt = layout.hit_test_point(Point::new(10.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
        assert_eq!(pt.is_trailing_hit, false);
        let pt = layout.hit_test_point(Point::new(14.0, 0.0));
        assert_eq!(pt.metrics.text_position, 9);
        assert_eq!(pt.is_trailing_hit, false);
        let pt = layout.hit_test_point(Point::new(18.0, 0.0));
        assert_eq!(pt.metrics.text_position, 9);
        assert_eq!(pt.is_trailing_hit, false);
        let pt = layout.hit_test_point(Point::new(19.0, 0.0));
        assert_eq!(pt.metrics.text_position, 9);
        assert_eq!(pt.is_trailing_hit, false);
        let pt = layout.hit_test_point(Point::new(30.0, 0.0));
        assert_eq!(pt.metrics.text_position, 10);
        assert_eq!(pt.is_trailing_hit, false);
    }

    #[test]
    fn test_count_until_utf16() {
        // Notes on this input:
        // 5 code points
        // 5 utf-16 code units
        // 10 utf-8 code units (2/1/3/3/1)
        // 3 graphemes
        let input = "é\u{0023}\u{FE0F}\u{20E3}1"; // #️⃣

        assert_eq!(count_until_utf16(input,0), Some(0));
        assert_eq!(count_until_utf16(input,1), Some(2));
        assert_eq!(count_until_utf16(input,2), Some(3));
        assert_eq!(count_until_utf16(input,3), Some(6));
        assert_eq!(count_until_utf16(input,4), Some(9));
        assert_eq!(count_until_utf16(input,5), None);
    }
}
