//! Text functionality for Piet svg backend

use std::{
    collections::HashSet,
    fs, io,
    ops::RangeBounds,
    sync::{Arc, Mutex},
};

use font_kit::{
    handle::Handle,
    source::{Source, SystemSource},
    sources::{mem::MemSource, multi::MultiSource},
};
use piet::kurbo::{Point, Rect, Size};
use piet::{
    Color, Error, FontFamily, FontStyle, FontWeight, HitTestPoint, HitTestPosition, LineMetric,
    TextAlignment, TextAttribute, TextStorage,
};
use rustybuzz::{Face, UnicodeBuffer};

type Result<T> = std::result::Result<T, Error>;

/// SVG text (partially implemented)
#[derive(Clone)]
pub struct Text {
    source: Arc<Mutex<MultiSource>>,
    /// Fonts we have seen this frame, and so need to embed in the SVG.
    ///
    /// We only include named font families - system defaults like SANS_SERIF are assumed to be
    /// present on the target system.
    pub(crate) seen_fonts: Arc<Mutex<HashSet<FontFace>>>,
}

impl Default for Text {
    fn default() -> Self {
        Self {
            source: Arc::new(Mutex::new(MultiSource::from_sources(vec![
                Box::new(SystemSource::new()),
                Box::new(MemSource::empty()),
            ]))),
            seen_fonts: Arc::new(Mutex::new(HashSet::new())),
        }
    }
}

impl Text {
    pub(crate) fn font_data(&self, face: &FontFace) -> Result<Arc<Vec<u8>>> {
        let handle = self
            .source
            .lock()
            .unwrap()
            .select_best_match(&[face.to_fk_family()], &face.to_props())
            .map_err(|_| Error::FontLoadingFailed)?;
        load_font_data(handle).map_err(|_| Error::FontLoadingFailed)
    }
}

impl piet::Text for Text {
    type TextLayout = TextLayout;
    type TextLayoutBuilder = TextLayoutBuilder;

    fn font_family(&mut self, family_name: &str) -> Option<FontFamily> {
        use font_kit::{family_name::FamilyName, properties::Properties};

        if self
            .source
            .lock()
            .unwrap()
            .select_best_match(&[FamilyName::Title(family_name.into())], &Properties::new())
            .is_ok()
        {
            Some(FontFamily::new_unchecked(family_name))
        } else {
            None
        }
    }

    fn load_font(&mut self, data: &[u8]) -> Result<FontFamily> {
        let mut multi_source = self.source.lock().unwrap();
        let source = multi_source
            .find_source_mut::<MemSource>()
            .expect("mem source");
        let font = source
            .add_font(Handle::Memory {
                bytes: Arc::new(data.into()),
                font_index: 0,
            })
            .map_err(|_| Error::FontLoadingFailed)?;
        Ok(FontFamily::new_unchecked(font.family_name()))
    }

    fn new_text_layout(&mut self, text: impl TextStorage) -> TextLayoutBuilder {
        TextLayoutBuilder::new(text, self.clone())
    }
}

pub struct TextLayoutBuilder {
    text: Arc<dyn TextStorage>,
    alignment: TextAlignment,
    font_face: FontFace,
    font_size: f64,
    text_color: Color,
    underline: bool,
    strikethrough: bool,
    max_width: f64,
    ctx: Text,
}

impl TextLayoutBuilder {
    fn new(text: impl TextStorage, ctx: Text) -> Self {
        Self {
            text: Arc::new(text),
            alignment: TextAlignment::default(),
            font_size: 12.,
            font_face: FontFace::default(),
            text_color: Color::BLACK,
            underline: false,
            strikethrough: false,
            max_width: f64::INFINITY,
            ctx,
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
            TextAttribute::FontFamily(font) => self.font_face.family = font,
            TextAttribute::FontSize(size) => self.font_size = size,
            TextAttribute::Weight(weight) => self.font_face.weight = weight,
            TextAttribute::TextColor(color) => self.text_color = color,
            TextAttribute::Style(style) => self.font_face.style = style,
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
    pub(crate) font_face: FontFace,
    pub(crate) text_color: Color,
    pub(crate) underline: bool,
    pub(crate) strikethrough: bool,
    size: Size,
}

impl TextLayout {
    /// Because we can't know what the rasterized output will look like (because the SVG could be
    /// displayed on another computer), we use the host computer to give 'best-guess' results for
    /// measurements. These are not guaranteed to be the same for when the SVG is rendered (e.g.
    /// will depend on available fonts, conformance of renderer, DPI, etc), but it is the best we
    /// can do.
    fn from_builder(builder: TextLayoutBuilder) -> Result<Self> {
        let face_bytes = builder
            .font_face
            .load(&*builder.ctx.source.lock().unwrap())?;
        let mut face = Face::from_slice(&face_bytes, 0).ok_or(Error::FontLoadingFailed)?;
        // number of pixels in a point
        // I think we're OK to assume 96 DPI, because the actual SVG renderer will scale for HIDPI
        // displays.
        const DPI: f64 = 96.;
        const POINTS_PER_INCH: f64 = 72.;
        let px_per_em = DPI / POINTS_PER_INCH * builder.font_size;
        let px_per_unit = px_per_em / face.units_per_em() as f64;
        face.set_pixels_per_em(Some((px_per_em as u16, px_per_em as u16)));

        let mut uni = UnicodeBuffer::new();

        // shape the full text
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
            font_face: builder.font_face,
            font_size: builder.font_size,
            text_color: builder.text_color,
            underline: builder.underline,
            strikethrough: builder.strikethrough,
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
        self.size().to_rect()
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

/// All the info required to indentify a font face. Basically, everythinge except the size.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Default)]
pub(crate) struct FontFace {
    pub family: FontFamily,
    pub weight: FontWeight,
    pub style: FontStyle,
}

impl FontFace {
    /// Load raw font data for `self`.
    fn load(&self, source: &impl Source) -> Result<Arc<Vec<u8>>> {
        load_font_data(self.find_handle(source)?).map_err(|_| Error::FontLoadingFailed)
    }

    fn find_handle(&self, source: &impl Source) -> Result<Handle> {
        source
            .select_best_match(&[self.to_fk_family()], &self.to_props())
            .map_err(|_| Error::FontLoadingFailed)
    }

    fn to_fk_family(&self) -> font_kit::family_name::FamilyName {
        use font_kit::family_name::FamilyName;
        if self.family == FontFamily::SANS_SERIF || self.family == FontFamily::SYSTEM_UI {
            FamilyName::SansSerif
        } else if self.family == FontFamily::SERIF {
            FamilyName::Serif
        } else if self.family == FontFamily::MONOSPACE {
            FamilyName::Monospace
        } else {
            FamilyName::Title(self.family.name().to_owned())
        }
    }

    fn to_props(&self) -> font_kit::properties::Properties {
        use font_kit::properties::{Properties, Style};

        let mut props = Properties::new();
        props.weight.0 = self.weight.to_raw() as f32;
        props.style = match self.style {
            FontStyle::Regular => Style::Normal,
            FontStyle::Italic => Style::Italic,
        };
        props
    }
}

pub(crate) fn load_font_data(handle: Handle) -> io::Result<Arc<Vec<u8>>> {
    // Load font data
    Ok(match handle {
        Handle::Path { path, font_index } => {
            if font_index > 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "font collections not supported",
                ));
            }
            Arc::new(fs::read(path)?)
        }
        Handle::Memory { bytes, font_index } => {
            if font_index > 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "font collections not supported",
                ));
            }
            bytes
        }
    })
}
