//! Text related stuff for the coregraphics backend

use std::marker::PhantomData;

use core_foundation_sys::base::CFRange;
use core_graphics::base::CGFloat;
use core_graphics::context::CGContextRef;
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use core_graphics::path::CGPath;
use core_text::font::{self, CTFont};

use piet::kurbo::{Point, Size};
use piet::{
    Error, Font, FontBuilder, HitTestMetrics, HitTestPoint, HitTestTextPosition, LineMetric, Text,
    TextLayout, TextLayoutBuilder,
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
    pub(crate) frame: Option<Frame>,
    // distance from the top of the frame to the baseline of each line
    pub(crate) line_y_positions: Vec<f64>,
    /// offsets in utf8 of lines
    line_offsets: Vec<usize>,
    pub(crate) frame_size: Size,
    width_constraint: f64,
}

pub struct CoreGraphicsTextLayoutBuilder(CoreGraphicsTextLayout);

pub struct CoreGraphicsText<'a>(PhantomData<&'a ()>);

impl<'a> CoreGraphicsText<'a> {
    /// Create a new factory that satisfies the piet `Text` trait.
    #[allow(clippy::new_without_default)]
    pub fn new() -> CoreGraphicsText<'a> {
        CoreGraphicsText(PhantomData)
    }
}

impl<'a> Text for CoreGraphicsText<'a> {
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

    #[allow(clippy::float_cmp)]
    fn update_width(&mut self, new_width: impl Into<Option<f64>>) -> Result<(), Error> {
        let width = new_width.into().unwrap_or(f64::INFINITY);
        if width.ceil() != self.width_constraint.ceil() {
            let constraints = CGSize::new(width as CGFloat, CGFloat::INFINITY);
            let char_range = self.attr_string.range();
            let (frame_size, _) = self.framesetter.suggest_frame_size(char_range, constraints);
            let rect = CGRect::new(&CGPoint::new(0.0, 0.0), &frame_size);
            let path = CGPath::from_rect(rect, None);
            self.width_constraint = width;
            let frame = self.framesetter.create_frame(char_range, &path);
            let line_count = frame.get_lines().len();
            let line_origins = frame.get_line_origins(CFRange::init(0, line_count));
            self.line_y_positions = line_origins
                .iter()
                .map(|l| frame_size.height - l.y)
                .collect();
            self.frame = Some(frame);
            self.frame_size = Size::new(frame_size.width, frame_size.height);
            self.rebuild_line_offsets();
        }
        Ok(())
    }

    fn line_text(&self, line_number: usize) -> Option<&str> {
        self.line_range(line_number)
            .map(|(start, end)| unsafe { self.string.get_unchecked(start..end) })
    }

    fn line_metric(&self, line_number: usize) -> Option<LineMetric> {
        let lines = self.unwrap_frame().get_lines();
        let line = lines.get(line_number.min(isize::max_value() as usize) as isize)?;
        let line = Line::new(&line);
        let typo_bounds = line.get_typographic_bounds();
        let (start_offset, end_offset) = self.line_range(line_number)?;
        let text = self.line_text(line_number)?;
        //FIXME: this is just ascii whitespace
        let trailing_whitespace = text
            .as_bytes()
            .iter()
            .rev()
            .take_while(|b| match b {
                b' ' | b'\t' | b'\n' | b'\r' => true,
                _ => false,
            })
            .count();
        let height = typo_bounds.ascent + typo_bounds.descent + typo_bounds.leading;
        // this may not be exactly right, but i'm also not sure we ever use this?
        //  see https://stackoverflow.com/questions/5511830/how-does-line-spacing-work-in-core-text-and-why-is-it-different-from-nslayoutm
        let cumulative_height =
            (self.line_y_positions[line_number] + typo_bounds.descent + typo_bounds.leading).ceil();
        Some(LineMetric {
            start_offset,
            end_offset,
            trailing_whitespace,
            baseline: typo_bounds.ascent,
            height,
            cumulative_height,
        })
    }

    fn line_count(&self) -> usize {
        self.line_y_positions.len()
    }

    // given a point on the screen, return an offset in the text, basically
    fn hit_test_point(&self, point: Point) -> HitTestPoint {
        let mut line_num = self
            .line_y_positions
            .iter()
            .position(|y| y >= &point.y)
            // if we're past the last line, use the last line
            .unwrap_or_else(|| self.line_y_positions.len().saturating_sub(1));
        // because y_positions is the position of the baseline, check that we don't
        // fall between the preceding baseline and that line's descent
        if line_num > 0 {
            let prev_line = self.unwrap_frame().get_line(line_num - 1).unwrap();
            let typo_bounds = Line::new(&&prev_line).get_typographic_bounds();
            if self.line_y_positions[line_num - 1] + typo_bounds.descent >= point.y {
                line_num -= 1;
            }
        }
        let line: Line = self
            .unwrap_frame()
            .get_line(line_num)
            .map(Into::into)
            .unwrap();
        let fake_y = self.line_y_positions[line_num];
        // map that back into our inverted coordinate space
        let fake_y = -(self.frame_size.height - fake_y);
        let point_in_string_space = CGPoint::new(point.x, fake_y);
        let offset_utf16 = line.get_string_index_for_position(point_in_string_space);
        let offset = match offset_utf16 {
            // this is 'kCFNotFound'.
            // if nothing is found just go end of string? should this be len - 1? do we have an
            // implicit newline at end of file? so many mysteries
            -1 => self.string.len(),
            n if n >= 0 => {
                let utf16_range = line.get_string_range();
                let utf8_range = self.line_range(line_num).unwrap();
                let line_txt = self.line_text(line_num).unwrap();
                let rel_offset = (n - utf16_range.location) as usize;
                let mut off16 = 0;
                let mut off8 = 0;
                for c in line_txt.chars() {
                    if rel_offset == off16 {
                        break;
                    }
                    off16 += c.len_utf16();
                    off8 += c.len_utf8();
                }
                utf8_range.0 + off8
            }
            // some other value; should never happen
            _ => panic!("gross violation of api contract"),
        };

        let typo_bounds = line.get_typographic_bounds();
        let is_inside_y = point.y >= 0. && point.y <= self.frame_size.height;
        let is_inside_x = point.x >= 0. && point.x <= typo_bounds.width;

        HitTestPoint {
            metrics: HitTestMetrics {
                text_position: offset,
            },
            is_inside: is_inside_x && is_inside_y,
        }
    }

    fn hit_test_text_position(&self, offset: usize) -> Option<HitTestTextPosition> {
        let line_num = match self.line_offsets.binary_search(&offset) {
            Ok(line) => line.saturating_sub(1),
            Err(line) => line.saturating_sub(1),
        };
        let line: Line = self.unwrap_frame().get_line(line_num)?.into();
        let text = self.line_text(line_num)?;

        let offset_remainder = offset - self.line_offsets.get(line_num)?;
        let off16: usize = text[..offset_remainder].chars().map(char::len_utf16).sum();
        let line_range = line.get_string_range();
        let char_idx = line_range.location + off16 as isize;
        let (x_pos, _) = line.get_offset_for_string_index(char_idx);
        let y_pos = self.line_y_positions[line_num];
        Some(HitTestTextPosition {
            point: Point::new(x_pos, y_pos),
            metrics: HitTestMetrics {
                text_position: offset,
            },
        })
    }
}

impl CoreGraphicsTextLayout {
    fn new(font: &CoreGraphicsFont, text: &str, width_constraint: f64) -> Self {
        let string = AttributedString::new(text, &font.0);
        let framesetter = Framesetter::new(&string);

        let mut layout = CoreGraphicsTextLayout {
            string: text.into(),
            attr_string: string,
            framesetter,
            // all of this is correctly set in `update_width` below
            frame: None,
            frame_size: Size::ZERO,
            line_y_positions: Vec::new(),
            // NaN to ensure we always execute code in update_width
            width_constraint: f64::NAN,
            line_offsets: Vec::new(),
        };
        layout.update_width(width_constraint).unwrap();
        layout
    }

    pub(crate) fn draw(&self, ctx: &mut CGContextRef) {
        self.unwrap_frame().0.draw(ctx)
    }

    #[inline]
    fn unwrap_frame(&self) -> &Frame {
        self.frame.as_ref().expect("always inited in ::new")
    }

    /// for each line in a layout, determine its offset in utf8.
    #[allow(clippy::while_let_on_iterator)]
    fn rebuild_line_offsets(&mut self) {
        let lines = self.unwrap_frame().get_lines();

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
#[allow(clippy::float_cmp)]
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

    #[test]
    fn metrics() {
        let text = "🤡:\na string\nwith a number \n of lines";
        let a_font = font::new_from_name("Helvetica", 16.0).unwrap();
        let layout = CoreGraphicsTextLayout::new(&CoreGraphicsFont(a_font), text, f64::INFINITY);
        let line1 = layout.line_metric(0).unwrap();
        assert_eq!(line1.start_offset, 0);
        assert_eq!(line1.end_offset, 6);
        assert_eq!(line1.trailing_whitespace, 1);
        layout.line_metric(1);

        let line3 = layout.line_metric(2).unwrap();
        assert_eq!(line3.start_offset, 15);
        assert_eq!(line3.end_offset, 30);
        assert_eq!(line3.trailing_whitespace, 2);

        let line4 = layout.line_metric(3).unwrap();
        assert_eq!(layout.line_text(3), Some(" of lines"));
        assert_eq!(line4.trailing_whitespace, 0);

        let total_height = layout.frame_size.height;
        assert_eq!(line4.cumulative_height, total_height);

        assert!(layout.line_metric(4).is_none());
    }

    // test that at least we're landing on the correct line
    #[test]
    fn basic_hit_testing() {
        let text = "1\n😀\n8\nA";
        let a_font = font::new_from_name("Helvetica", 16.0).unwrap();
        let layout = CoreGraphicsTextLayout::new(&CoreGraphicsFont(a_font), text, f64::INFINITY);
        let p1 = layout.hit_test_point(Point::ZERO);
        assert_eq!(p1.metrics.text_position, 0);
        assert!(p1.is_inside);
        let p2 = layout.hit_test_point(Point::new(2.0, 19.0));
        assert_eq!(p2.metrics.text_position, 0);
        assert!(p2.is_inside);

        //FIXME: figure out correct multiline behaviour; this should be
        //before the newline, but other backends aren't doing this right now either?

        //let p3 = layout.hit_test_point(Point::new(50.0, 10.0));
        //assert_eq!(p3.metrics.text_position, 1);
        //assert!(!p3.is_inside);

        let p4 = layout.hit_test_point(Point::new(4.0, 25.0));
        assert_eq!(p4.metrics.text_position, 2);
        assert!(p4.is_inside);

        let p5 = layout.hit_test_point(Point::new(2.0, 83.0));
        assert_eq!(p5.metrics.text_position, 9);
        assert!(p5.is_inside);

        let p6 = layout.hit_test_point(Point::new(10.0, 83.0));
        assert_eq!(p6.metrics.text_position, 10);
        assert!(p6.is_inside);
    }

    #[test]
    fn hit_test_end_of_single_line() {
        let text = "hello";
        let a_font = font::new_from_name("Helvetica", 16.0).unwrap();
        let layout = CoreGraphicsTextLayout::new(&CoreGraphicsFont(a_font), text, f64::INFINITY);
        let pt = layout.hit_test_point(Point::new(0.0, 5.0));
        assert_eq!(pt.metrics.text_position, 0);
        assert_eq!(pt.is_inside, true);
        let next_to_last = layout.frame_size.width - 10.0;
        let pt = layout.hit_test_point(Point::new(next_to_last, 0.0));
        assert_eq!(pt.metrics.text_position, 4);
        assert_eq!(pt.is_inside, true);
        let pt = layout.hit_test_point(Point::new(100.0, 5.0));
        assert_eq!(pt.metrics.text_position, 5);
        assert_eq!(pt.is_inside, false);
    }

    #[test]
    fn hit_test_text_position() {
        let text = "aaaaa\nbbbbb";
        let a_font = font::new_from_name("Helvetica", 16.0).unwrap();
        let layout = CoreGraphicsTextLayout::new(&CoreGraphicsFont(a_font), text, f64::INFINITY);
        let p1 = layout.hit_test_text_position(0).unwrap();
        assert_eq!(p1.point, Point::new(0.0, 16.0));

        let p1 = layout.hit_test_text_position(7).unwrap();
        assert_eq!(p1.point.y, 36.0);
        // just the general idea that this is the second character
        assert!(p1.point.x > 5.0 && p1.point.x < 15.0);
    }
}
