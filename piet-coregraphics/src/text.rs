//! Text related stuff for the coregraphics backend

use std::ops::{Range, RangeBounds};

use core_foundation::base::TCFType;
use core_foundation::dictionary::CFMutableDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_foundation_sys::base::CFRange;
use core_graphics::base::CGFloat;
use core_graphics::color::CGColor;
use core_graphics::context::CGContextRef;
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use core_graphics::path::CGPath;
use core_text::{font, font::CTFont, font_descriptor, string_attributes};

use piet::kurbo::{Point, Size};
use piet::util;
use piet::{
    Error, Font, FontBuilder, FontWeight, HitTestMetrics, HitTestPoint, HitTestTextPosition,
    LineMetric, Text, TextAlignment, TextAttribute, TextLayout, TextLayoutBuilder,
};

use crate::ct_helpers::{AttributedString, Frame, Framesetter, Line};

//TODO: this should be a CTFontDescriptor maybe?
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

/// Building text layouts for `CoreGraphics`.
pub struct CoreGraphicsTextLayoutBuilder {
    width: f64,
    alignment: TextAlignment,
    default_font: CoreGraphicsFont,
    text: String,
    /// the end bound up to which we have already added attrs to our AttributedString
    last_resolved_pos: usize,
    last_resolved_utf16: usize,
    attr_string: AttributedString,
    font: Span<CoreGraphicsFont>,
    size: Span<f64>,
    weight: Span<FontWeight>,
    italic: Span<bool>,
}

/// during construction, `Span`s represent font attributes that have been applied
/// to ranges of the text; these are combined into coretext font objects as the
/// layout is built.
struct Span<T> {
    payload: T,
    range: Range<usize>,
}

impl<T> Span<T> {
    fn new(payload: T, range: Range<usize>) -> Self {
        Span { payload, range }
    }
}

impl CoreGraphicsTextLayoutBuilder {
    /// ## Note
    ///
    /// The implementation of this has a few particularities.
    ///
    /// The main Foundation type for representing a rich text string is NSAttributedString
    /// (CFAttributedString in CoreFoundation); however not all attributes are set
    /// directly. Attributes that implicate font selection (such as size, weight, etc)
    /// are all part of the string's 'font' attribute; we can't set them individually.
    ///
    /// To make this work, we keep track of the active value for each of the relevant
    /// attributes. Each span of the string with a common set of these values is assigned
    /// the appropriate concrete font as the attributes are added.
    ///
    /// This behaviour relies on the condition that spans are added in non-decreasing
    /// start order. The algorithm is quite simple; whenever a new attribute of one
    /// of the relevant types is added, we know that spans in the string up to
    /// the start of the newly added span can no longer be changed, and we can resolve them.
    fn add(&mut self, attr: TextAttribute<CoreGraphicsFont>, range: Range<usize>) {
        // Some attributes are 'standalone' and can just be added to the attributed string
        // immediately.
        if matches!(&attr, TextAttribute::ForegroundColor(_) | TextAttribute::Underline) {
            return self.add_immediately(attr, range);
        }

        debug_assert!(
            range.start >= self.last_resolved_pos,
            "attributes must be added with non-decreasing start positions"
        );

        self.resolve_up_to(range.start);
        // Other attributes need to be handled incrementally, since they all participate
        // in creating the CTFont objects
        match attr {
            TextAttribute::Font(font) => self.font = Span::new(font, range),
            TextAttribute::Weight(weight) => self.weight = Span::new(weight, range),
            TextAttribute::Size(size) => self.size = Span::new(size, range),
            TextAttribute::Italic => self.italic = Span::new(true, range),
            _ => unreachable!(),
        }
    }

    fn add_immediately(&mut self, attr: TextAttribute<CoreGraphicsFont>, range: Range<usize>) {
        let utf16_start = util::count_utf16(&self.text[..range.start]);
        let utf16_len = util::count_utf16(&self.text[range]);
        let range = CFRange::init(utf16_start as isize, utf16_len as isize);

        let (key, value) = unsafe {
            match attr {
                TextAttribute::ForegroundColor(color) => {
                    let (r, g, b, a) = color.as_rgba();
                    let color = CGColor::rgb(r, g, b, a);
                    (
                        string_attributes::kCTForegroundColorAttributeName,
                        color.as_CFType(),
                    )
                }
                TextAttribute::Underline => {
                    #[allow(non_upper_case_globals)]
                    const kCTUnderlineStyleSingle: i32 = 0x01;
                    (
                        string_attributes::kCTUnderlineStyleAttributeName,
                        CFNumber::from(kCTUnderlineStyleSingle).as_CFType(),
                    )
                }
                _ => unreachable!(),
            }
        };
        self.attr_string.inner.set_attribute(range, key, &value);
    }

    fn finalize(&mut self) {
        self.resolve_up_to(self.text.len());
    }

    /// Add all font attributes up to a boundary.
    fn resolve_up_to(&mut self, resolve_end: usize) {
        let mut next_span_end = self.last_resolved_pos;
        while next_span_end < resolve_end {
            next_span_end = self.next_span_end(resolve_end);
            if next_span_end > self.last_resolved_pos {
                let range_end_utf16 =
                    util::count_utf16(&self.text[self.last_resolved_pos..next_span_end]);
                let range =
                    CFRange::init(self.last_resolved_utf16 as isize, range_end_utf16 as isize);
                let font = self.current_font();
                unsafe {
                    self.attr_string.inner.set_attribute(
                        range,
                        string_attributes::kCTFontAttributeName,
                        &font,
                    );
                }
                self.last_resolved_pos = next_span_end;
                self.last_resolved_utf16 += range_end_utf16;
                self.update_after_adding_span();
            }
        }
    }

    /// Given the end of a range, return the min of that value and the ends of
    /// any existing spans.
    ///
    /// ## Invariant
    ///
    /// It is an invariant that the end range of any `FontAttr` is greater than
    /// `self.last_resolved_pos`
    fn next_span_end(&self, max: usize) -> usize {
        self.font
            .range
            .end
            .min(self.size.range.end)
            .min(self.weight.range.end)
            .min(max)
    }

    /// returns the fully constructed font object, including weight and size.
    ///
    /// This is stateful; it depends on the current attributes being correct
    /// for the range that begins at `self.last_resolved_pos`.
    fn current_font(&self) -> CTFont {
        //TODO: this is where caching would happen, if we were implementing caching;
        //store a tuple of attributes resolves to a generated CTFont.
        unsafe {
            let weight_key = CFString::wrap_under_create_rule(font_descriptor::kCTFontWeightTrait);
            let weight = convert_to_coretext(self.weight.payload);
            let family_key =
                CFString::wrap_under_create_rule(font_descriptor::kCTFontFamilyNameAttribute);
            let family = self.font.payload.0.family_name();

            let traits_key =
                CFString::wrap_under_create_rule(font_descriptor::kCTFontTraitsAttribute);
            let mut traits = CFMutableDictionary::new();
            traits.set(weight_key, weight.as_CFType());
            if self.italic.payload {
                let symbolic_traits_key =
                    CFString::wrap_under_create_rule(font_descriptor::kCTFontSymbolicTrait);
                let symbolic_traits = CFNumber::from(font_descriptor::kCTFontItalicTrait as i32);
                traits.set(symbolic_traits_key, symbolic_traits.as_CFType());
            }

            let mut attributes = CFMutableDictionary::new();
            attributes.set(traits_key, traits.as_CFType());
            attributes.set(family_key, CFString::new(&family).as_CFType());

            let descriptor = font_descriptor::new_from_attributes(&attributes.to_immutable());
            font::new_from_descriptor(&descriptor, self.size.payload)
        }
    }

    /// After we have added a span, check to see if any of our attributes are no
    /// longer active.
    ///
    /// This is stateful; it requires that `self.last_resolved_pos` has been just updated
    /// to reflect the end of the span just added.
    fn update_after_adding_span(&mut self) {
        let remaining_range = self.last_resolved_pos..self.text.len();
        if self.font.range.end == self.last_resolved_pos {
            self.font = Span::new(self.default_font.clone(), remaining_range.clone());
        }

        if self.weight.range.end == self.last_resolved_pos {
            self.weight = Span::new(FontWeight::REGULAR, remaining_range.clone());
        }

        if self.size.range.end == self.last_resolved_pos {
            self.size = Span::new(self.default_font.0.pt_size(), remaining_range.clone());
        }

        if self.italic.range.end == self.last_resolved_pos {
            self.italic = Span::new(false, remaining_range);
        }
    }
}

/// coretext uses a float in the range -1.0..=1.0, which has a non-linear mapping
/// to css-style weights. This is a fudge, adapted from QT:
///
/// https://git.sailfishos.org/mer-core/qtbase/commit/9ba296cc4cefaeb9d6c5abc2e0c0b272f2288733#1b84d1913347bd20dd0a134247f8cd012a646261_44_55
//TODO: a better solution would be piecewise linear interpolation between these values
fn convert_to_coretext(weight: FontWeight) -> CFNumber {
    match weight.to_raw() {
        0..=199 => -0.8,
        200..=299 => -0.6,
        300..=399 => -0.4,
        400..=499 => 0.0,
        500..=599 => 0.23,
        600..=699 => 0.3,
        700..=799 => 0.4,
        800..=899 => 0.56,
        _ => 0.62,
    }
    .into()
}

#[derive(Clone)]
pub struct CoreGraphicsText;

impl CoreGraphicsText {
    /// Create a new factory that satisfies the piet `Text` trait.
    #[allow(clippy::new_without_default)]
    pub fn new() -> CoreGraphicsText {
        CoreGraphicsText
    }
}

impl Text for CoreGraphicsText {
    type Font = CoreGraphicsFont;
    type FontBuilder = CoreGraphicsFontBuilder;
    type TextLayout = CoreGraphicsTextLayout;
    type TextLayoutBuilder = CoreGraphicsTextLayoutBuilder;

    fn new_font_by_name(&mut self, name: &str, size: f64) -> Self::FontBuilder {
        CoreGraphicsFontBuilder(font::new_from_name(name, size).ok())
    }

    fn system_font(&mut self, size: f64) -> Self::Font {
        CoreGraphicsFont(crate::ct_helpers::system_font(size))
    }

    fn new_text_layout(
        &mut self,
        font: &Self::Font,
        text: &str,
        width: impl Into<Option<f64>>,
    ) -> Self::TextLayoutBuilder {
        let width = width.into().unwrap_or(f64::INFINITY);
        CoreGraphicsTextLayoutBuilder::new(font, text, width)
    }
}

impl Font for CoreGraphicsFont {}

impl FontBuilder for CoreGraphicsFontBuilder {
    type Out = CoreGraphicsFont;

    fn build(self) -> Result<Self::Out, Error> {
        self.0.map(CoreGraphicsFont).ok_or(Error::MissingFont)
    }
}

impl CoreGraphicsTextLayoutBuilder {
    fn new(font: &CoreGraphicsFont, text: &str, width: f64) -> Self {
        let attr_string = AttributedString::new(text);
        let range_all = 0..text.len();
        CoreGraphicsTextLayoutBuilder {
            width,
            alignment: TextAlignment::default(),
            default_font: font.clone(),
            text: text.to_string(),
            last_resolved_pos: 0,
            last_resolved_utf16: 0,
            attr_string,
            font: Span::new(font.clone(), range_all.clone()),
            size: Span::new(font.0.pt_size(), range_all.clone()),
            italic: Span::new(false, range_all.clone()),
            weight: Span::new(FontWeight::REGULAR, range_all),
        }
    }
}

impl TextLayoutBuilder for CoreGraphicsTextLayoutBuilder {
    type Out = CoreGraphicsTextLayout;
    type Font = CoreGraphicsFont;

    fn alignment(mut self, alignment: piet::TextAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    fn add_attribute(
        mut self,
        range: impl RangeBounds<usize>,
        attribute: impl Into<TextAttribute<Self::Font>>,
    ) -> Self {
        let range = util::resolve_range(range, self.text.len());
        let attribute = attribute.into();
        self.add(attribute, range);
        self
    }

    fn build(mut self) -> Result<Self::Out, Error> {
        self.finalize();
        self.attr_string.set_alignment(self.alignment);
        Ok(CoreGraphicsTextLayout::new(
            self.text,
            self.attr_string,
            self.width,
        ))
    }
}

impl TextLayout for CoreGraphicsTextLayout {
    fn width(&self) -> f64 {
        self.frame_size.width
    }

    fn size(&self) -> Size {
        self.frame_size
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
        // this may not be exactly right, but i'm also not sure we ever use this?
        //  see https://stackoverflow.com/questions/5511830/how-does-line-spacing-work-in-core-text-and-why-is-it-different-from-nslayoutm
        let ascent = (typo_bounds.ascent + 0.5).floor();
        let descent = (typo_bounds.descent + 0.5).floor();
        let leading = (typo_bounds.leading + 0.5).floor();
        let height = ascent + descent + leading;
        let y_offset = self.line_y_positions[line_number] - ascent;
        let cumulative_height = self.line_y_positions[line_number] + descent + leading;
        #[allow(deprecated)]
        Some(LineMetric {
            start_offset,
            end_offset,
            trailing_whitespace,
            baseline: typo_bounds.ascent,
            height,
            cumulative_height,
            y_offset,
        })
    }

    fn line_count(&self) -> usize {
        self.line_y_positions.len()
    }

    // given a point on the screen, return an offset in the text, basically
    fn hit_test_point(&self, point: Point) -> HitTestPoint {
        if self.line_y_positions.is_empty() {
            return HitTestPoint {
                metrics: HitTestMetrics { text_position: 0 },
                is_inside: false,
            };
        }

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
            -1 => self.string.len(),
            n if n >= 0 => {
                let utf16_range = line.get_string_range();
                let utf8_range = self.line_range(line_num).unwrap();
                let line_txt = self.line_text(line_num).unwrap();
                let rel_offset = (n - utf16_range.location) as usize;
                utf8_range.0
                    + util::count_until_utf16(line_txt, rel_offset)
                        .unwrap_or_else(|| line_txt.len())
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
        let line_num = self.line_number_for_utf8_offset(offset);
        let line: Line = self.unwrap_frame().get_line(line_num)?.into();
        let text = self.line_text(line_num)?;

        let offset_remainder = offset - self.line_offsets.get(line_num)?;
        let off16: usize = util::count_utf16(&text[..offset_remainder]);
        let line_range = line.get_string_range();
        let char_idx = line_range.location + off16 as isize;
        let x_pos = line.get_offset_for_string_index(char_idx);
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
    fn new(text: String, attr_string: AttributedString, width_constraint: f64) -> Self {
        let framesetter = Framesetter::new(&attr_string);

        let mut layout = CoreGraphicsTextLayout {
            string: text,
            attr_string,
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

    fn line_number_for_utf8_offset(&self, offset: usize) -> usize {
        match self.line_offsets.binary_search(&offset) {
            Ok(line) => line,
            Err(line) => line.saturating_sub(1),
        }
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
        let layout =
            CoreGraphicsTextLayoutBuilder::new(&CoreGraphicsFont(a_font), text, f64::INFINITY)
                .build()
                .unwrap();
        assert_eq!(layout.line_text(0), Some("hi\n"));
        assert_eq!(layout.line_text(1), Some("i'm\n"));
        assert_eq!(layout.line_text(2), Some("😀 four\n"));
        assert_eq!(layout.line_text(3), Some("lines"));
    }

    #[test]
    fn metrics() {
        let text = "🤡:\na string\nwith a number \n of lines";
        let a_font = font::new_from_name("Helvetica", 16.0).unwrap();
        let layout =
            CoreGraphicsTextLayoutBuilder::new(&CoreGraphicsFont(a_font), text, f64::INFINITY)
                .build()
                .unwrap();

        let line1 = layout.line_metric(0).unwrap();
        assert_eq!(line1.range(), 0..6);
        assert_eq!(line1.trailing_whitespace, 1);
        layout.line_metric(1);

        let line3 = layout.line_metric(2).unwrap();
        assert_eq!(line3.range(), 15..30);
        assert_eq!(line3.trailing_whitespace, 2);

        let line4 = layout.line_metric(3).unwrap();
        assert_eq!(layout.line_text(3), Some(" of lines"));
        assert_eq!(line4.trailing_whitespace, 0);

        let total_height = layout.frame_size.height;
        assert_eq!(line4.y_offset + line4.height, total_height);

        assert!(layout.line_metric(4).is_none());
    }

    // test that at least we're landing on the correct line
    #[test]
    fn basic_hit_testing() {
        let text = "1\n😀\n8\nA";
        let a_font = font::new_from_name("Helvetica", 16.0).unwrap();
        let layout =
            CoreGraphicsTextLayoutBuilder::new(&CoreGraphicsFont(a_font), text, f64::INFINITY)
                .build()
                .unwrap();

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
        let layout =
            CoreGraphicsTextLayoutBuilder::new(&CoreGraphicsFont(a_font), text, f64::INFINITY)
                .build()
                .unwrap();
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
    fn hit_test_point_empty_string() {
        let text = "";
        let a_font = font::new_from_name("Helvetica", 16.0).unwrap();
        let layout =
            CoreGraphicsTextLayoutBuilder::new(&CoreGraphicsFont(a_font), text, f64::INFINITY)
                .build()
                .unwrap();
        let pt = layout.hit_test_point(Point::new(0.0, 0.0));
        assert_eq!(pt.metrics.text_position, 0);
    }

    #[test]
    fn hit_test_text_position() {
        let text = "aaaaa\nbbbbb";
        let a_font = font::new_from_name("Helvetica", 16.0).unwrap();
        let layout =
            CoreGraphicsTextLayoutBuilder::new(&CoreGraphicsFont(a_font), text, f64::INFINITY)
                .build()
                .unwrap();
        let p1 = layout.hit_test_text_position(0).unwrap();
        assert_eq!(p1.point, Point::new(0.0, 16.0));

        let p1 = layout.hit_test_text_position(7).unwrap();
        assert_eq!(p1.point.y, 36.0);
        // just the general idea that this is the second character
        assert!(p1.point.x > 5.0 && p1.point.x < 15.0);
    }

    #[test]
    fn hit_test_text_position_astral_plane() {
        let text = "👾🤠\n🤖🎃👾";
        let a_font = font::new_from_name("Helvetica", 16.0).unwrap();
        let layout =
            CoreGraphicsTextLayoutBuilder::new(&CoreGraphicsFont(a_font), text, f64::INFINITY)
                .build()
                .unwrap();
        let p0 = layout.hit_test_text_position(4).unwrap();
        let p1 = layout.hit_test_text_position(8).unwrap();
        let p2 = layout.hit_test_text_position(13).unwrap();

        assert!(p1.point.x > p0.point.x);
        assert!(p1.point.y == p0.point.y);
        assert!(p2.point.y > p1.point.y);
    }
}
