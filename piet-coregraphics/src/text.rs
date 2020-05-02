//! Text related stuff for the coregraphics backend

use core_foundation_sys::base::CFRange;
use core_graphics::base::CGFloat;
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use core_graphics::path::CGPath;
use core_text::font::{self, CTFont};

use piet::kurbo::{Point, Size};
use piet::{
    Error, Font, FontBuilder, HitTestPoint, HitTestTextPosition, LineMetric, Text, TextLayout,
    TextLayoutBuilder,
};

use crate::ct_helpers::{AttributedString, Frame, Framesetter, Line};

// inner is an nsfont.
#[derive(Debug, Clone)]
pub struct CoreGraphicsFont(CTFont);

pub struct CoreGraphicsFontBuilder(Option<CTFont>);

#[derive(Clone)]
pub struct CoreGraphicsTextLayout {
    string: String,
    attr_string: AttributedString,
    framesetter: Framesetter,
    pub(crate) frame: Frame,
    line_origins: Vec<CGPoint>,
    /// offsets in utf8 of lines
    line_offsets: Vec<usize>,
    pub(crate) frame_size: Size,
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
        let layout = CoreGraphicsTextLayout::new(font, text, width_constraint);
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
            let char_range = self.attr_string.range();
            let (frame_size, _) = self.framesetter.suggest_frame_size(char_range, constraints);
            let rect = CGRect::new(&CGPoint::new(0.0, 0.0), &frame_size);
            let path = CGPath::from_rect(rect, None);
            self.width_constraint = width;
            self.frame = self.framesetter.create_frame(char_range, &path);
            let line_count = self.frame.get_lines().len();
            self.line_origins = self.frame.get_line_origins(CFRange::init(0, line_count));
            self.frame_size = Size::new(frame_size.width, frame_size.height);
            self.rebuild_line_offsets();
        }
        Ok(())
    }

    fn line_text(&self, line_number: usize) -> Option<&str> {
        self.line_range(line_number)
            .map(|(start, end)| unsafe { self.string.get_unchecked(start..end) })
    }

    fn line_metric(&self, _line_number: usize) -> Option<LineMetric> {
        unimplemented!()
    }

    fn line_count(&self) -> usize {
        self.line_origins.len()
    }

    fn hit_test_point(&self, _point: Point) -> HitTestPoint {
        unimplemented!()
    }

    fn hit_test_text_position(&self, _text_position: usize) -> Option<HitTestTextPosition> {
        unimplemented!()
    }
}

impl CoreGraphicsTextLayout {
    fn new(font: &CoreGraphicsFont, text: &str, width_constraint: f64) -> Self {
        let constraints = CGSize::new(width_constraint as CGFloat, CGFloat::INFINITY);
        let string = AttributedString::new(text, &font.0);

        let framesetter = Framesetter::new(&string);
        let char_range = string.range();

        let (frame_size, _) = framesetter.suggest_frame_size(char_range, constraints);
        let rect = CGRect::new(&CGPoint::new(0.0, 0.0), &frame_size);
        let path = CGPath::from_rect(rect, None);
        let frame = framesetter.create_frame(char_range, &path);
        let lines = frame.get_lines();
        let line_origins = frame.get_line_origins(CFRange::init(0, lines.len()));

        let frame_size = Size::new(frame_size.width, frame_size.height);

        let mut layout = CoreGraphicsTextLayout {
            string: text.into(),
            attr_string: string,
            framesetter,
            frame,
            frame_size,
            line_origins,
            width_constraint,
            line_offsets: Vec::new(),
        };
        layout.rebuild_line_offsets();
        layout
    }

    /// for each line in a layout, determine its offset in utf8.
    fn rebuild_line_offsets(&mut self) {
        let lines = self.frame.get_lines();

        let utf16_line_offsets = lines.iter().map(|l| {
            let line = Line::new(&l);
            let range = line.get_string_range();
            range.location as usize
        });

        let mut chars = self.string.chars();
        let mut cur_16 = 0;
        let mut cur_8 = 0;

        self.line_offsets = utf16_line_offsets
            .map(|off_16| {
                if off_16 == 0 {
                    return 0;
                }
                while let Some(c) = chars.next() {
                    cur_16 += c.len_utf16();
                    cur_8 += c.len_utf8();
                    if cur_16 == off_16 {
                        return cur_8;
                    }
                }
                panic!("error calculating utf8 offsets");
            })
            .collect::<Vec<_>>();
    }

    fn line_range(&self, line: usize) -> Option<(usize, usize)> {
        if line <= self.line_count() {
            let start = self.line_offsets[line];
            let end = if line == self.line_count() - 1 {
                self.string.len()
            } else {
                self.line_offsets[line + 1]
            };
            Some((start, end))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn line_offsets() {
        let text = "hi\ni'm\n😀 four\nlines";
        let a_font = font::new_from_name("Helvetica", 16.0).unwrap();
        let layout = CoreGraphicsTextLayout::new(&CoreGraphicsFont(a_font), text, f64::INFINITY);
        assert_eq!(layout.line_text(0), Some("hi\n"));
        assert_eq!(layout.line_text(1), Some("i'm\n"));
        assert_eq!(layout.line_text(2), Some("😀 four\n"));
        assert_eq!(layout.line_text(3), Some("lines"));
    }
}
