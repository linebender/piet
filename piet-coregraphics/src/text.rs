//! Text related stuff for the coregraphics backend

use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::attributed_string::CFMutableAttributedString;
use core_foundation::base::TCFType;
use core_foundation::dictionary::CFDictionaryRef;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;

use core_foundation_sys::base::CFRange;
use core_graphics::base::CGFloat;
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use core_graphics::path::CGPath;
use core_text::font::{self, CTFont};
use core_text::frame::{CTFrame, CTFrameRef};
use core_text::framesetter::{CTFramesetter, CTFramesetterRef};
use core_text::line::CTLine;
use core_text::string_attributes;

use piet::kurbo::{Point, Size};
use piet::{
    Error, Font, FontBuilder, HitTestPoint, HitTestTextPosition, LineMetric, Text, TextLayout,
    TextLayoutBuilder,
};

// inner is an nsfont.
#[derive(Debug, Clone)]
pub struct CoreGraphicsFont(CTFont);

pub struct CoreGraphicsFontBuilder(Option<CTFont>);

#[derive(Clone)]
pub struct CoreGraphicsTextLayout {
    framesetter: CTFramesetter,
    pub(crate) frame: CTFrame,
    pub(crate) frame_size: Size,
    line_count: usize,
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
        let width = width.into().unwrap_or(f64::INFINITY);
        let constraints = CGSize::new(width as CGFloat, CGFloat::INFINITY);
        let mut string = CFMutableAttributedString::new();
        let range = CFRange::init(0, 0);
        string.replace_str(&CFString::new(text), range);

        let str_len = string.char_len();
        let char_range = CFRange::init(0, str_len);
        unsafe {
            string.set_attribute(
                char_range,
                string_attributes::kCTFontAttributeName,
                font.0.clone(),
            );
            string.set_attribute::<CFNumber>(
                char_range,
                string_attributes::kCTForegroundColorFromContextAttributeName,
                1i32.into(),
            );
        }

        let framesetter = CTFramesetter::new_with_attributed_string(string.as_concrete_TypeRef());

        let mut fit_range = CFRange::init(0, 0);
        let frame_size = unsafe {
            CTFramesetterSuggestFrameSizeWithConstraints(
                framesetter.as_concrete_TypeRef(),
                char_range,
                std::ptr::null(),
                constraints,
                &mut fit_range,
            )
        };

        let rect = CGRect::new(&CGPoint::new(0.0, 0.0), &frame_size);
        let path = CGPath::from_rect(rect, None);
        let frame = framesetter.create_frame(char_range, &path);

        let lines: CFArray<CTLine> =
            unsafe { TCFType::wrap_under_get_rule(CTFrameGetLines(frame.as_concrete_TypeRef())) };
        let line_count = lines.len() as usize;

        let frame_size = Size::new(frame_size.width, frame_size.height);
        let layout = CoreGraphicsTextLayout {
            framesetter,
            frame,
            frame_size,
            line_count,
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

    fn update_width(&mut self, _new_width: impl Into<Option<f64>>) -> Result<(), Error> {
        unimplemented!()
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

#[link(name = "CoreText", kind = "framework")]
extern "C" {
    fn CTFramesetterSuggestFrameSizeWithConstraints(
        framesetter: CTFramesetterRef,
        string_range: CFRange,
        frame_attributes: CFDictionaryRef,
        constraints: CGSize,
        fitRange: *mut CFRange,
    ) -> CGSize;

    fn CTFrameGetLines(frame: CTFrameRef) -> CFArrayRef;
}
