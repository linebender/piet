use piet::{
    kurbo::{Point, Rect, Size},
    Color, Font, FontBuilder, HitTestPoint, HitTestPosition, LineMetric, Text, TextAlignment,
    TextAttribute, TextLayout, TextLayoutBuilder,
};
use std::ops::RangeBounds;

#[derive(Default)]
pub struct PfTextLayoutBuilder {
    text: String,
    color: Option<Color>,
    font: Option<PfFont>,
    width: Option<f64>,
}

impl PfTextLayoutBuilder {
    fn resolve(self) -> PfTextLayout {
        PfTextLayout {
            font: self.font.unwrap_or_default(),
            color: self.color.unwrap_or(Color::BLACK),
            text: self.text,
            width: self.width.unwrap_or(f64::INFINITY),
        }
    }
}

impl TextLayoutBuilder for PfTextLayoutBuilder {
    type Out = PfTextLayout;
    type Font = PfFont;

    fn max_width(self, width: f64) -> Self {
        self
    }

    fn alignment(self, alignment: TextAlignment) -> Self {
        self
    }

    fn default_attribute(mut self, attribute: impl Into<TextAttribute<Self::Font>>) -> Self {
        match attribute.into() {
            TextAttribute::Size(size) => self.font = Some(PfFont { size }),
            TextAttribute::ForegroundColor(color) => self.color = Some(color),
            _ => todo!(),
        }
        self
    }

    fn range_attribute(
        self,
        range: impl RangeBounds<usize>,
        attribute: impl Into<TextAttribute<Self::Font>>,
    ) -> Self {
        self
    }

    fn build(self) -> Result<Self::Out, piet::Error> {
        Ok(self.resolve())
    }

    fn font(self, font: Self::Font, font_size: f64) -> Self {
        self
    }
}

#[derive(Clone)]
pub struct PfTextLayout {
    pub(crate) font: PfFont,
    pub(crate) color: Color,
    pub(crate) text: String,
    pub(crate) width: f64,
}

impl TextLayout for PfTextLayout {
    fn width(&self) -> f64 {
        todo!()
    }

    fn size(&self) -> Size {
        todo!()
    }

    fn image_bounds(&self) -> Rect {
        todo!()
    }

    fn update_width(&mut self, new_width: impl Into<Option<f64>>) -> Result<(), piet::Error> {
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

    fn hit_test_text_position(&self, text_position: usize) -> Option<HitTestPosition> {
        todo!()
    }
}

#[derive(Clone)]
pub struct PfText;

impl Text for PfText {
    type FontBuilder = PfFontBuilder;
    type Font = PfFont;
    type TextLayoutBuilder = PfTextLayoutBuilder;
    type TextLayout = PfTextLayout;
    fn new_font_by_name(&mut self, name: &str, size: f64) -> Self::FontBuilder {
        Self::FontBuilder {
            //name: name.to_string(),
            size,
        }
    }

    fn system_font(&mut self, size: f64) -> Self::Font {
        todo!()
    }

    fn new_text_layout(&mut self, text: &str) -> Self::TextLayoutBuilder {
        PfTextLayoutBuilder {
            text: text.to_string(),
            ..Default::default()
        }
    }
}

pub struct PfFontBuilder {
    size: f64,
}

impl FontBuilder for PfFontBuilder {
    type Out = PfFont;
    fn build(self) -> Result<Self::Out, piet::Error> {
        Ok(PfFont {
            //name: self.name,
            size: self.size,
        })
    }
}

#[derive(Clone)]
pub struct PfFont {
    pub(crate) size: f64,
}

impl Default for PfFont {
    fn default() -> Self {
        PfFont { size: 12.0 }
    }
}

impl Font for PfFont {}
