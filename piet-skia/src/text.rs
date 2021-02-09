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

use crate::simple_text::*;

#[derive(Clone)]
pub struct SkiaText {}

impl SkiaText {
    pub fn new() -> Self {
        SkiaText{}
    }
}

#[derive(Clone, Debug)]
pub enum SkiaTextLayout {
    Simple(SkiaSimpleTextLayout),
    Paragraph(ParagraphTextLayout),
}

#[derive(Clone)]
pub struct ParagraphTextLayout {
    pub(crate) fg_color: Color,
    size: Size,
    pub text: Rc<dyn TextStorage>,
    pub width: f32,
    // Paragraph doesn't support Clone trait
    pub paragraph: Rc<Paragraph>,
}

impl fmt::Debug for ParagraphTextLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SkiaTextLayoutBuilder")
            .field("fg_color", &self.fg_color)
            .field("text", &self.text.as_str())
            .field("width", &self.width)
            .finish()
    }
}

impl fmt::Debug for SkiaTextLayoutBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SkiaTextLayoutBuilder")
            .field("text", &self.text.as_str())
            .field("width_constraint", &self.width_constraint)
            .finish()
    }
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
        let layout = if self.width_constraint.is_finite() {
            let mut font_collection = FontCollection::new();
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
            let width = paragraph
                .get_line_metrics()
                .iter()
                .map(|l| l.width)
                .max_by(|x, y| x.abs().partial_cmp(&y.abs()).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or(0.) as f32;

            // note: if you do paragraph.layout(width) again it will wrap last word
            // on each line because it's exact size as width_constraint
            SkiaTextLayout::Paragraph(ParagraphTextLayout{
                fg_color,
                size: Size::ZERO,
                text: self.text,
                width,
                paragraph: Rc::new(paragraph),
            })
        } else {
            let mut paint = Paint::default();
            let fg_color = self.defaults.fg_color;
            paint.set_color(crate::convert_color(fg_color.clone()));
            let mut font = Font::default(); // take font from TextLayout
            let size = self.defaults.font_size; 
            font.set_size(size as f32);
            let (width, rect) = font.measure_str(self.text.as_str(), None);
            let line_metrics = calculate_line_metrics(self.text.as_str(), &font);
            let height = line_metrics.last().map(|l| 
                l.y_offset + l.bounds.height() as f64
            ).unwrap_or_else(||{
                let (_, metrics) = font.metrics();
                let height = (metrics.descent - metrics.ascent + metrics.leading) as f64;
                height
            });
            let width = line_metrics.iter().map(|l| l.bounds.width())
                .max_by(|x, y| x.partial_cmp(&y).unwrap()).unwrap_or(0.);
            let size = Size::new(width as f64, height);
            SkiaTextLayout::Simple(SkiaSimpleTextLayout{
                line_metrics,
                fg_color,
                size,
                font: Rc::new(font),
                text: self.text,
            })
        };
        Ok(layout)
    }
}
impl TextLayout for SkiaTextLayout {
    fn size(&self) -> Size {
        match self {
            SkiaTextLayout::Paragraph(paragraph) => {
                paragraph.size()
            }
            SkiaTextLayout::Simple(simple) => {
                simple.size()
            }
        }
    }

    fn trailing_whitespace_width(&self) -> f64 {
        unimplemented!();
    }

    fn image_bounds(&self) -> Rect {
        match self {
            SkiaTextLayout::Paragraph(paragraph) => {
                paragraph.image_bounds()
            }
            SkiaTextLayout::Simple(simple) => {
                simple.image_bounds()
            }
        }
    }

    fn text(&self) -> &str {
        match self {
            SkiaTextLayout::Paragraph(paragraph) => {
                paragraph.text()
            }
            SkiaTextLayout::Simple(simple) => {
                simple.text()
            }
        }
    }

    fn line_text(&self, line_number: usize) -> Option<&str> {
        match self {
            SkiaTextLayout::Paragraph(paragraph) => {
                paragraph.line_text(line_number)
            }
            SkiaTextLayout::Simple(simple) => {
                simple.line_text(line_number)
            }
        }
    }

    fn line_metric(&self, line_number: usize) -> Option<LineMetric> {
        match self {
            SkiaTextLayout::Paragraph(paragraph) => {
                paragraph.line_metric(line_number)
            }
            SkiaTextLayout::Simple(simple) => {
                simple.line_metric(line_number)
            }
        }
    }

    fn line_count(&self) -> usize {
        match self {
            SkiaTextLayout::Paragraph(paragraph) => {
                paragraph.line_count()
            }
            SkiaTextLayout::Simple(simple) => {
                simple.line_count()
            }
        }
   }

    fn hit_test_point(&self, point: Point) -> HitTestPoint { 
        match self {
            SkiaTextLayout::Paragraph(paragraph) => {
                paragraph.hit_test_point(point)
            }
            SkiaTextLayout::Simple(simple) => {
                simple.hit_test_point(point)
            }
        }
    }

    fn hit_test_text_position(&self, idx: usize) -> HitTestPosition {
        match self {
            SkiaTextLayout::Paragraph(paragraph) => {
                paragraph.hit_test_text_position(idx)
            }
            SkiaTextLayout::Simple(simple) => {
                simple.hit_test_text_position(idx)
            }
        }
    }
}

impl TextLayout for ParagraphTextLayout {
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
