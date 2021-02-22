use std::ops::RangeBounds;
use std::rc::Rc;

use piet::kurbo::{Point, Rect, Size};
use piet::{
    util, Color, Error, FontFamily, HitTestPoint, HitTestPosition, LineMetric, Text,
    TextAttribute, TextLayout, TextLayoutBuilder, TextStorage, FontWeight
};
use skia_safe::{Font, FontMgr, Paint, Contains};
use skia_safe::textlayout::{ParagraphBuilder, ParagraphStyle, FontCollection, Paragraph, LineMetricsVector, TextStyle, RectWidthStyle, RectHeightStyle};
use skia_safe::typeface::Typeface;
use skia_safe::font_style::{FontStyle, Weight, Width, Slant};

use std::fmt;

use crate::simple_text::*;

#[derive(Clone)]
pub struct SkiaText;

impl SkiaText {
    pub fn new() -> Self {
        SkiaText
    }
}

#[derive(Clone, Debug)]
pub enum SkiaTextLayout {
    Simple(SkiaSimpleTextLayout),
    Paragraph(ParagraphTextLayout),
}

pub struct ParagraphTextLayout {
    pub text: Rc<dyn TextStorage>,
    pub width: f32,
    // Paragraph doesn't support Clone trait so we need to store some info to rebuild it
    // we store Rc here cause we need to clone this data too
    defaults: Rc<util::LayoutDefaults>,
    pub paragraph: Paragraph,
}


impl Clone for ParagraphTextLayout {
    fn clone(&self) -> Self {
        Self {
            text: self.text.clone(),
            width: self.width.clone(),
            defaults: self.defaults.clone(),
            paragraph: build_paragraph(self.text.as_ref(), &self.defaults, self.width),
        }
    }
}

impl fmt::Debug for ParagraphTextLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SkiaTextLayoutBuilder")
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

fn build_typeface(defaults: &util::LayoutDefaults) -> Typeface {
    let mut font_collection = FontCollection::new();
    let font_mngr = FontMgr::new();
    font_collection.set_default_font_manager(font_mngr, defaults.font.name());
    let width = Width::NORMAL; // no options provided via piet
    let weight: Weight = (defaults.weight.to_raw() as i32).into();
    let slant = match defaults.style {
        piet::FontStyle::Regular => {
            Slant::Upright
        }
        piet::FontStyle::Italic => {
            Slant::Italic
        }
    };
    let font_style = FontStyle::new(weight, width, slant);
    Typeface::new(defaults.font.name(), font_style).unwrap()
}

// It's convinient to have a separate method for creating paragraph, cause it doesn't have Clone
// TODO all font related data should be moved into struct fields at some point 
fn build_paragraph(text: &str, defaults: &util::LayoutDefaults, width_constraint: f32) -> Paragraph {
    let mut paint = Paint::default();
    let mut font_collection = FontCollection::new();
    let font_mngr = FontMgr::new();
    font_collection.set_default_font_manager(font_mngr, defaults.font.name());
    let mut paragraph_style = ParagraphStyle::new();
    let mut text_style = TextStyle::new();
    let fg_color = defaults.fg_color.clone();
    let typeface = build_typeface(defaults);
    text_style.set_typeface(Some(typeface));
    text_style.set_font_size(defaults.font_size as f32);
    paint.set_color(crate::convert_color(fg_color));
    text_style.set_foreground_color(paint);
    paragraph_style.set_text_style(&text_style);
    let mut paragraph_builder = ParagraphBuilder::new(&paragraph_style, font_collection);
    paragraph_builder.add_text(text);
    let mut paragraph = paragraph_builder.build();
    paragraph.layout(width_constraint);
    paragraph
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

    fn build(self) -> Result<Self::Out, Error> {
        let layout = if self.width_constraint.is_finite() {
            let paragraph = build_paragraph(self.text.as_str(), &self.defaults, self.width_constraint as f32);
            let width = paragraph
                .get_line_metrics()
                .iter()
                .map(|l| l.width)
                .max_by(|x, y| x.abs().partial_cmp(&y.abs()).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or(0.) as f32;

            // note: if you do paragraph.layout(width) again it will wrap last word
            // on each line because it's exact size as width_constraint
            SkiaTextLayout::Paragraph(ParagraphTextLayout{
                text: self.text,
                width,
                defaults: Rc::new(self.defaults),
                paragraph,
            })
        } else {
            let mut paint = Paint::default();
            let font = {
                let size = self.defaults.font_size;
                let typeface = build_typeface(&self.defaults);
                Font::new(typeface, Some(size as f32))
            };
            let fg_color = self.defaults.fg_color;
            paint.set_color(crate::convert_color(fg_color.clone()));
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

impl ParagraphTextLayout {
    pub fn fg_color(&self) -> Color {
        self.defaults.fg_color.clone()
    }

    // this is the most efficient way for updating width, because skia's paragraph perform cashing
    // for layout function
    fn update_width(&mut self, new_width: f32) {
        self.width = new_width;
        self.paragraph.layout(self.width);
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
        self.size().to_rect()
    }

    fn text(&self) -> &str {
        &self.text
    }

    fn line_text(&self, _line_number: usize) -> Option<&str> { 
        unimplemented!();
    }

    fn line_metric(&self, line_number: usize) -> Option<LineMetric> {
        self.paragraph.get_line_metrics().as_slice().get(line_number).map(|line_metric| {
            LineMetric {
                start_offset: line_metric.start_index,
                end_offset: line_metric.end_index,
                trailing_whitespace: line_metric.end_index - line_metric.end_excluding_whitespaces,
                baseline: line_metric.baseline,
                height: line_metric.height,
                y_offset: line_metric.baseline - line_metric.ascent
            }
        })
    }

    fn line_count(&self) -> usize {
        self.paragraph.line_number()
    }

    fn hit_test_point(&self, point: Point) -> HitTestPoint {
        let skia_point = skia_safe::Point::new(point.x as f32, point.y as f32);
        let position = self.paragraph.get_glyph_position_at_coordinate(skia_point);
        let idx = position.position as usize;
        let text_boxes = self.paragraph.get_rects_for_range(idx..(idx + 1), RectHeightStyle::Tight, RectWidthStyle::Tight);
        let mut contains = false;
        for text_box in text_boxes.iter() {
            if text_box.rect.contains(skia_point) {
                contains = true
            }
        }
        HitTestPoint::new(idx, contains)
    }

    fn hit_test_text_position(&self, idx: usize) -> HitTestPosition {
        let idx = idx.min(self.text.len());
        let clipped_idx = if self.text.len() == idx {idx - 1} else {idx}; // handling case when idx = text length
        let text_boxes = self.paragraph.get_rects_for_range(clipped_idx..(clipped_idx + 1), RectHeightStyle::Tight, RectWidthStyle::Tight);
        let res = if let Some(glyph_box) = text_boxes.iter().next() {
            let point = if idx == self.text.len() {
                Point::new(glyph_box.rect.right as f64, glyph_box.rect.top as f64)
            } else {
                Point::new(glyph_box.rect.left as f64, glyph_box.rect.top as f64)
            };
            let center = &glyph_box.rect.center();
            let mut i = 0;
            let mut line_number = 0;
            while let Some(metrics) = self.line_metric(i) {
                if center.y as f64 > metrics.y_offset && (center.y as f64) < metrics.y_offset + metrics.height {
                    line_number = i;
                }
                i += 1;
            }
            HitTestPosition::new(point, line_number)
        } else {
            let info = format!("{} -- {}", self.text.as_str(), self.text.as_str().len());
            dbg!(&info);
            dbg!(idx);
            //panic!();
            HitTestPosition::new(Point::new(0., 0.), 0)
        };
        //dbg!(idx, &res);
        res
        //let glyph_box = text_boxes.iter().next().expect(&format!("hit_test_text_position called with idx={} out of boundary {}", idx, self.text.as_str().len()));
        //let point = Point::new(glyph_box.rect.left as f64, glyph_box.rect.top as f64);
        //HitTestPosition::new(point, 0)
    }
}
