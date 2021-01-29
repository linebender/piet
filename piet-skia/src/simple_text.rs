use std::ops::RangeBounds;
use std::rc::Rc;

use piet::kurbo::{Point, Rect, Size};
use piet::{
    util, Color, Error, FontFamily, FontStyle, HitTestPoint, HitTestPosition, LineMetric, Text,
    TextAttribute, TextLayout, TextLayoutBuilder, TextStorage,
};
use skia_safe::{Path, Font, FontMgr, Paint};
use skia_safe::textlayout::{ParagraphBuilder, ParagraphStyle, FontCollection, TextStyle, Paragraph};

use std::fmt;

#[derive(Clone)]
pub struct SkiaSimpleText {}

impl SkiaSimpleText {
    pub fn new() -> Self {
        SkiaSimpleText{}
    }
}

#[derive(Clone)]
pub struct SkiaSimpleTextLayout {
    pub(crate) fg_color: Color,
    pub(crate) size: Size,
    // skia doesn't support Clone trait for font...
    pub font: Rc<Font>,
    pub text: Rc<dyn TextStorage>,
    pub width: f32,
    pub rect: skia_safe::Rect
}

impl fmt::Debug for SkiaSimpleTextLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SkiaTextLayoutBuilder")
            .field("fg_color", &self.fg_color)
            .field("font", &self.font)
            .field("text", &self.text.as_str())
            .field("width", &self.width)
            .finish()
    }
}

impl fmt::Debug for SkiaSimpleTextLayoutBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SkiaTextLayoutBuilder")
            .field("text", &self.text.as_str())
            .field("width_constraint", &self.width_constraint)
            .finish()
    }
}

pub struct SkiaSimpleTextLayoutBuilder {
    text: Rc<dyn TextStorage>,
    defaults: util::LayoutDefaults,
    width_constraint: f64,
}

impl Text for SkiaSimpleText {
    type TextLayout = SkiaSimpleTextLayout;
    type TextLayoutBuilder = SkiaSimpleTextLayoutBuilder;

    fn font_family(&mut self, family_name: &str) -> Option<FontFamily> {
        Some(FontFamily::new_unchecked(family_name))
    }

    fn load_font(&mut self, _data: &[u8]) -> Result<FontFamily, Error> {
        Err(Error::NotSupported)
    }

    fn new_text_layout(&mut self, text: impl TextStorage) -> Self::TextLayoutBuilder {
        SkiaSimpleTextLayoutBuilder{
            defaults: util::LayoutDefaults::default(),
            text: Rc::new(text),
            width_constraint: f64::INFINITY,
        }
    }
}

impl TextLayoutBuilder for SkiaSimpleTextLayoutBuilder {
    type Out = SkiaSimpleTextLayout;

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
        let mut paint = Paint::default();
        let fg_color = self.defaults.fg_color;
        paint.set_color(crate::convert_color(fg_color.clone()));
        let mut font = Font::default(); // take font from TextLayout
        let size = self.defaults.font_size; 
        font.set_size(size as f32);
        let (width, rect) = font.measure_str(self.text.as_str(), None);
        // note: if you do paragraph.layout(width) again it will wrap last word
        // on each line because it's exact size as width_constraint
        Ok(SkiaSimpleTextLayout{
            fg_color,
            size: Size::ZERO,
            font: Rc::new(font),
            text: self.text,
            width,
            rect
        })
    }
}

impl TextLayout for SkiaSimpleTextLayout {
    fn size(&self) -> Size {
        let size = Size::new(self.width as f64, self.rect.height() as f64);
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
        // TODO
        HitTestPoint::new(0, false)
        //if point.y > self.paragraph.height() {
        //   return HitTestPoint::default() 
        //}
        //let width = self.paragraph
        //    .get_line_metrics()
        //    .iter()
        //    .map(|l| l.width);
    }

    fn hit_test_text_position(&self, idx: usize) -> HitTestPosition {
        // TODO
        HitTestPosition::new(Point::new(0., 0.), 0)
    }
}
