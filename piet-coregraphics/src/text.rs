//! Text related stuff for the coregraphics backend

use core_graphics::base::CGFloat;
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use core_graphics::path::CGPath;
use core_text::font::{self, CTFont};

use piet::kurbo::{Point, Size};
use piet::{
    Error, Font, FontBuilder, HitTestPoint, HitTestTextPosition, LineMetric, Text, TextLayout,
    TextLayoutBuilder,
};

use crate::ct_helpers::{AttributedString, Frame, Framesetter};

// inner is an nsfont.
#[derive(Debug, Clone)]
pub struct CoreGraphicsFont(CTFont);

pub struct CoreGraphicsFontBuilder(Option<CTFont>);

#[derive(Clone)]
pub struct CoreGraphicsTextLayout {
    string: AttributedString,
    framesetter: Framesetter,
    pub(crate) frame: Frame,
    pub(crate) frame_size: Size,
    line_count: usize,
    width_constraint: f64,
}

pub struct CoreGraphicsTextLayoutBuilder(CoreGraphicsTextLayout);

pub struct CoreGraphicsText;

impl Text for CoreGraphicsText {
    type Font = CoreGraphicsFont;
    type FontBuilder = CoreGraphicsFontBuilder;
    type TextLayout = CoreGraphicsTextLayout;
    type TextLayoutBuilder = CoreGraphicsTextLayoutBuilder;

    fn new_font_by_name(&mut self, name: &str, size: f64) -> Self::FontBuilder {
        CoreGraphicsFontBuilder(font::new_from_name(name, size).ok())
    }

    fn new_text_layout(
        &mut self,
        font: &Self::Font,
        text: &str,
        width: impl Into<Option<f64>>,
    ) -> Self::TextLayoutBuilder {
        let width_constraint = width.into().unwrap_or(f64::INFINITY);
        let constraints = CGSize::new(width_constraint as CGFloat, CGFloat::INFINITY);
        let string = AttributedString::new(text, &font.0);

        let framesetter = Framesetter::new(&string);
        let char_range = string.range();

        let (frame_size, _) = framesetter.suggest_frame_size(char_range, constraints);
        let rect = CGRect::new(&CGPoint::new(0.0, 0.0), &frame_size);
        let path = CGPath::from_rect(rect, None);
        let frame = framesetter.create_frame(char_range, &path);
        let lines = frame.get_lines();
        let line_count = lines.len() as usize;

        let frame_size = Size::new(frame_size.width, frame_size.height);
        let layout = CoreGraphicsTextLayout {
            string,
            framesetter,
            frame,
            frame_size,
            line_count,
            width_constraint,
        };
        CoreGraphicsTextLayoutBuilder(layout)
    }
}

impl Font for CoreGraphicsFont {}

impl FontBuilder for CoreGraphicsFontBuilder {
    type Out = CoreGraphicsFont;

    fn build(self) -> Result<Self::Out, Error> {
        self.0.map(CoreGraphicsFont).ok_or(Error::MissingFont)
    }
}

impl TextLayoutBuilder for CoreGraphicsTextLayoutBuilder {
    type Out = CoreGraphicsTextLayout;

    fn build(self) -> Result<Self::Out, Error> {
        Ok(self.0)
    }
}

impl TextLayout for CoreGraphicsTextLayout {
    fn width(&self) -> f64 {
        self.frame_size.width
    }

    fn update_width(&mut self, new_width: impl Into<Option<f64>>) -> Result<(), Error> {
        let width = new_width.into().unwrap_or(f64::INFINITY);
        if width != self.width_constraint {
            let constraints = CGSize::new(width as CGFloat, CGFloat::INFINITY);
            let char_range = self.string.range();
            let (frame_size, _) = self.framesetter.suggest_frame_size(char_range, constraints);
            let rect = CGRect::new(&CGPoint::new(0.0, 0.0), &frame_size);
            let path = CGPath::from_rect(rect, None);
            self.width_constraint = width;
            self.frame = self.framesetter.create_frame(char_range, &path);
            self.line_count = self.frame.get_lines().len() as usize;
            self.frame_size = Size::new(frame_size.width, frame_size.height);
        }
        Ok(())
    }

    fn line_text(&self, _line_number: usize) -> Option<&str> {
        unimplemented!()
    }

    fn line_metric(&self, _line_number: usize) -> Option<LineMetric> {
        unimplemented!()
    }

    fn line_count(&self) -> usize {
        self.line_count
    }

    fn hit_test_point(&self, _point: Point) -> HitTestPoint {
        unimplemented!()
    }

    fn hit_test_text_position(&self, _text_position: usize) -> Option<HitTestTextPosition> {
        unimplemented!()
    }
}
