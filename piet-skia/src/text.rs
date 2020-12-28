use std::ops::RangeBounds;
use std::rc::Rc;

use piet::kurbo::{Point, Rect, Size};
use piet::{
    util, Color, Error, FontFamily, FontStyle, HitTestPoint, HitTestPosition, LineMetric, Text,
    TextAttribute, TextLayout, TextLayoutBuilder, TextStorage,
};
use skia_safe::{Path, Font, FontMgr, Paint};
use skia_safe::textlayout::{ParagraphBuilder, ParagraphStyle, FontCollection, TextStyle, Paragraph};

#[derive(Clone)]
pub struct SkiaText {}

impl SkiaText {
    pub fn new() -> Self {
        SkiaText{}
    }
}

#[derive(Clone)]
pub struct SkiaTextLayout {
    pub(crate) fg_color: Color,
    size: Size,
    // skia doesn't support Clone trait for font...
    pub font: Rc<Font>,
    text: Rc<dyn TextStorage>,
    pub width: f32,
    // Paragraph doesn't support Clone trait
    pub paragraph: Rc<Paragraph>,
}

pub struct SkiaTextLayoutBuilder {
    text: Rc<dyn TextStorage>,
    defaults: util::LayoutDefaults,
    width_constraint: f64,
}

impl Text for SkiaText {
    type TextLayout = SkiaTextLayout;
    type TextLayoutBuilder = SkiaTextLayoutBuilder;

    fn font_family(&mut self, family_name: &str) -> Option<FontFamily> {
        Some(FontFamily::new_unchecked(family_name))
    }

    fn load_font(&mut self, _data: &[u8]) -> Result<FontFamily, Error> {
        Err(Error::NotSupported)
    }

    fn new_text_layout(&mut self, text: impl TextStorage) -> Self::TextLayoutBuilder {
        SkiaTextLayoutBuilder{
            defaults: util::LayoutDefaults::default(),
            text: Rc::new(text),
            width_constraint: f64::INFINITY,
        }
    }
}

impl TextLayoutBuilder for SkiaTextLayoutBuilder {
    type Out = SkiaTextLayout;

    fn max_width(mut self, width: f64) -> Self {
        self.width_constraint = width;
        self
    }

    fn alignment(self, _alignment: piet::TextAlignment) -> Self {
        self
    }

    fn default_attribute(mut self, attribute: impl Into<TextAttribute>) -> Self {
        self.defaults.set(attribute);
        self
    }

    fn range_attribute(
        self,
        _range: impl RangeBounds<usize>,
        _attribute: impl Into<TextAttribute>,
    ) -> Self {
        self
        // TODO
    }

    fn build(mut self) -> Result<Self::Out, Error> {
        let mut font_collection = FontCollection::new();
        // TODO it's possible to create it as OnceCell, and preload Fonts
        // that we might use
        let font_mngr = FontMgr::new();
        font_collection.set_default_font_manager(font_mngr, None);
        let paragraph_style = ParagraphStyle::new();
        let mut paragraph_builder = ParagraphBuilder::new(&paragraph_style, font_collection);
        let mut ts = TextStyle::new();
        ts.set_font_size(self.defaults.font_size as f32);
        let mut paint = Paint::default();
        let fg_color = self.defaults.fg_color;
        paint.set_color(crate::convert_color(fg_color.clone()));
        ts.set_foreground_color(paint);
        paragraph_builder.push_style(&ts);
        paragraph_builder.add_text(self.text.as_str());
        let mut paragraph = paragraph_builder.build();
        paragraph.layout(self.width_constraint as f32);
        // there is no API to get exact width in current skia paragraph
        let width = paragraph
            .get_line_metrics()
            .iter()
            .map(|l| l.width)
            .max_by(|x, y| x.abs().partial_cmp(&y.abs()).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.) as f32;

        // TODO add to paragraph
        let mut font = Font::default(); // take font from TextLayout
        let size = self.defaults.font_size;
        
        font.set_size(size as f32);
        // note: if you do paragraph.layout(width) again it will wrap last word
        // on each line because it's exact size as width_constraint
        Ok(SkiaTextLayout{ 
            fg_color,
            size: Size::ZERO,
            font: Rc::new(font),
            text: self.text,
            width,
            paragraph: Rc::new(paragraph),
        })
    }
}

impl TextLayout for SkiaTextLayout {
    fn size(&self) -> Size {
        let size = Size::new(self.width as f64, self.paragraph.height() as f64);
        size
    }

    fn trailing_whitespace_width(&self) -> f64 {
        unimplemented!();
    }

    fn image_bounds(&self) -> Rect {
        self.size.to_rect()
    }

    fn text(&self) -> &str {
        &self.text
    }

    fn line_text(&self, line_number: usize) -> Option<&str> {
        unimplemented!();
    }

    fn line_metric(&self, line_number: usize) -> Option<LineMetric> {
        // for now we can just support only one line text 
        let mut metrics = LineMetric::default();
        Some(metrics) // TODO
    }

    fn line_count(&self) -> usize {
        unimplemented!();
    }

    fn hit_test_point(&self, point: Point) -> HitTestPoint { 
        unimplemented!();
        //if point.y > self.paragraph.height() {
        //   return HitTestPoint::default() 
        //}
        //let width = self.paragraph
        //    .get_line_metrics()
        //    .iter()
        //    .map(|l| l.width);
    }

    fn hit_test_text_position(&self, idx: usize) -> HitTestPosition {
        unimplemented!();
    }
}
