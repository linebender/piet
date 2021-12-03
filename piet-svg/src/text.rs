//! Text functionality for Piet svg backend

use std::{fs, ops::RangeBounds, sync::Arc};

use piet::kurbo::{Point, Rect, Size};
use piet::{
    Color, Error, FontFamily, FontStyle, FontWeight, HitTestPoint, HitTestPosition, LineMetric,
    TextAlignment, TextAttribute, TextStorage,
};
use rustybuzz::{Face, UnicodeBuffer};

type Result<T> = std::result::Result<T, Error>;

/// SVG text (unimplemented)
#[derive(Debug, Clone)]
pub struct Text;

impl Text {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Text
    }
}

impl piet::Text for Text {
    type TextLayout = TextLayout;
    type TextLayoutBuilder = TextLayoutBuilder;

    fn font_family(&mut self, _family_name: &str) -> Option<FontFamily> {
        Some(FontFamily::default())
    }

    fn load_font(&mut self, _data: &[u8]) -> Result<FontFamily> {
        Ok(FontFamily::default())
    }

    fn new_text_layout(&mut self, text: impl TextStorage) -> TextLayoutBuilder {
        TextLayoutBuilder::new(text)
    }
}

pub struct TextLayoutBuilder {
    text: Arc<dyn TextStorage>,
    alignment: TextAlignment,
    font_family: FontFamily,
    font_size: f64,
    font_weight: FontWeight,
    font_style: FontStyle,
    text_color: Color,
    underline: bool,
    strikethrough: bool,
    max_width: f64,
}

impl TextLayoutBuilder {
    fn new(text: impl TextStorage) -> Self {
        Self {
            text: Arc::new(text),
            alignment: TextAlignment::default(),
            font_family: FontFamily::default(),
            font_size: 12.,
            font_weight: FontWeight::default(),
            font_style: FontStyle::default(),
            text_color: Color::BLACK,
            underline: false,
            strikethrough: false,
            max_width: f64::INFINITY,
        }
    }
}

impl piet::TextLayoutBuilder for TextLayoutBuilder {
    type Out = TextLayout;

    fn max_width(mut self, width: f64) -> Self {
        // This is totally ignored for now when measuring.
        self.max_width = width;
        self
    }

    fn alignment(mut self, alignment: piet::TextAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    fn default_attribute(mut self, attribute: impl Into<TextAttribute>) -> Self {
        match attribute.into() {
            TextAttribute::FontFamily(font) => self.font_family = font,
            TextAttribute::FontSize(size) => self.font_size = size,
            TextAttribute::Weight(weight) => self.font_weight = weight,
            TextAttribute::TextColor(color) => self.text_color = color,
            TextAttribute::Style(style) => self.font_style = style,
            TextAttribute::Underline(underline) => self.underline = underline,
            TextAttribute::Strikethrough(strikethrough) => self.strikethrough = strikethrough,
        }

        self
    }

    fn range_attribute(
        mut self,
        range: impl RangeBounds<usize>,
        attribute: impl Into<TextAttribute>,
    ) -> Self {
        if range.contains(&0) && range.contains(&(self.text.len() - 1)) {
            self = self.default_attribute(attribute)
        } else {
            // TODO non-full ranges are unsupported
        }
        self
    }

    fn build(self) -> Result<TextLayout> {
        TextLayout::from_builder(self)
    }
}

/// SVG text layout
#[derive(Clone)]
pub struct TextLayout {
    text: Arc<dyn TextStorage>,
    pub(crate) max_width: f64,
    pub(crate) alignment: TextAlignment,
    pub(crate) font_size: f64,
    pub(crate) font_family: FontFamily,
    pub(crate) font_weight: FontWeight,
    pub(crate) font_style: FontStyle,
    pub(crate) text_color: Color,
    pub(crate) underline: bool,
    pub(crate) strikethrough: bool,
    size: Size,
    face: (Arc<Vec<u8>>, u32),
}

impl TextLayout {
    /// Because we can't know what the rasterized output will look like (because the SVG could be
    /// displayed on another computer), we use the host computer to give 'best-guess' results for
    /// measurements. These are not guaranteed to be the same for when the SVG is rendered (e.g.
    /// will depend on available fonts, conformance of renderer, DPI, etc), but it is the best we
    /// can do.
    fn from_builder(builder: TextLayoutBuilder) -> Result<Self> {
        let (face_bytes, idx) = load_fontface(
            &builder.font_family,
            builder.font_weight,
            builder.font_style,
        )?;
        let mut face = Face::from_slice(&face_bytes, idx).ok_or(Error::FontLoadingFailed)?;
        let px_per_em = 96. / 72. * builder.font_size;
        let px_per_unit = px_per_em / face.units_per_em() as f64;
        face.set_pixels_per_em(Some((px_per_em as u16, px_per_em as u16)));
        // number of pixels in a point
        // 96 = dpi, 72 = points per inch

        let mut uni = UnicodeBuffer::new();

        // full text
        uni.push_str(builder.text.as_str());
        let layout = rustybuzz::shape(&face, &[], uni);
        let width = layout
            .glyph_positions()
            .iter()
            .map(|pos| pos.x_advance as f64)
            .sum::<f64>()
            * px_per_unit;
        let height = face.height() as f64 * px_per_unit;
        let size = Size { width, height };

        Ok(TextLayout {
            text: builder.text,
            max_width: builder.max_width,
            alignment: builder.alignment,
            font_family: builder.font_family,
            font_size: builder.font_size,
            font_weight: builder.font_weight,
            font_style: builder.font_style,
            text_color: builder.text_color,
            underline: builder.underline,
            strikethrough: builder.strikethrough,
            face: (face_bytes, idx),
            size,
        })
    }
}

impl piet::TextLayout for TextLayout {
    fn size(&self) -> Size {
        // TODO shape multiple rows
        self.size
    }

    fn trailing_whitespace_width(&self) -> f64 {
        // unimplemented
        0.
    }

    fn image_bounds(&self) -> Rect {
        Rect::from((Point::from((0., 0.)), self.size()))
    }

    fn line_text(&self, line_number: usize) -> Option<&str> {
        if line_number == 0 {
            Some(&self.text)
        } else {
            None
        }
    }

    fn line_metric(&self, line_number: usize) -> Option<LineMetric> {
        if line_number == 0 {
            Some(LineMetric {
                start_offset: 0,
                end_offset: self.text.len(),
                trailing_whitespace: self.text.len() - self.text.trim_end().len(),
                baseline: 0.,
                height: 0.,
                y_offset: 0.,
            })
        } else {
            None
        }
    }

    fn line_count(&self) -> usize {
        1
    }

    fn hit_test_point(&self, _point: Point) -> HitTestPoint {
        HitTestPoint::default()
    }

    fn hit_test_text_position(&self, _text_position: usize) -> HitTestPosition {
        HitTestPosition::default()
    }

    fn text(&self) -> &str {
        self.text.as_str()
    }
}

/// Returns (face data, idx).
fn load_fontface(
    family: &FontFamily,
    weight: FontWeight,
    style: FontStyle,
) -> Result<(Arc<Vec<u8>>, u32)> {
    use font_kit::{
        family_name::FamilyName,
        handle::Handle,
        properties::{Properties, Style},
    };

    let mut props = Properties::new();
    props.weight.0 = weight.to_raw() as f32;
    props.style = match style {
        FontStyle::Regular => Style::Normal,
        FontStyle::Italic => Style::Italic,
    };

    let source = font_kit::source::SystemSource::new();
    let font_handle = source
        .select_best_match(
            &[
                FamilyName::Title(family.name().to_string()),
                FamilyName::SansSerif,
            ],
            &props,
        )
        .map_err(|_| Error::FontLoadingFailed)?;
    Ok(match font_handle {
        Handle::Path { path, font_index } => {
            let bytes = fs::read(path).map_err(|_| Error::FontLoadingFailed)?;
            (Arc::new(bytes), font_index)
        }
        Handle::Memory { bytes, font_index } => (bytes, font_index),
    })
}
