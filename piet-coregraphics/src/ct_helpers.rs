//! Wrappers around CF/CT types, with nice interfaces.

use std::borrow::Cow;
use std::convert::TryInto;
use std::ops::Deref;

use core_foundation::{
    array::{CFArray, CFArrayRef, CFIndex},
    attributed_string::CFMutableAttributedString,
    base::TCFType,
    boolean::CFBoolean,
    string::{CFString, CFStringRef},
};
use core_foundation_sys::base::CFRange;
use core_graphics::{
    base::CGFloat,
    geometry::{CGPoint, CGSize},
    path::CGPathRef,
};
use core_text::{
    font::{kCTFontSystemFontType, CTFont, CTFontRef, CTFontUIFontType},
    frame::{CTFrame, CTFrameRef},
    framesetter::CTFramesetter,
    line::{CTLine, TypographicBounds},
    string_attributes,
};

#[derive(Clone)]
pub(crate) struct AttributedString(pub(crate) CFMutableAttributedString);
#[derive(Debug, Clone)]
pub(crate) struct Framesetter(CTFramesetter);
#[derive(Debug, Clone)]
pub(crate) struct Frame(pub(crate) CTFrame);
#[derive(Debug, Clone)]
pub(crate) struct Line<'a>(pub(crate) Cow<'a, CTLine>);

impl AttributedString {
    pub(crate) fn new(text: &str, font: &CTFont) -> Self {
        let mut string = CFMutableAttributedString::new();
        let range = CFRange::init(0, 0);
        string.replace_str(&CFString::new(text), range);

        let str_len = string.char_len();
        let char_range = CFRange::init(0, str_len);

        unsafe {
            string.set_attribute(char_range, string_attributes::kCTFontAttributeName, font);
            string.set_attribute::<CFBoolean>(
                char_range,
                string_attributes::kCTForegroundColorFromContextAttributeName,
                &CFBoolean::true_value(),
            );
        }
        AttributedString(string)
    }

    pub(crate) fn range(&self) -> CFRange {
        CFRange::init(0, self.0.char_len())
    }
}

impl Framesetter {
    pub(crate) fn new(attributed_string: &AttributedString) -> Self {
        Framesetter(CTFramesetter::new_with_attributed_string(
            attributed_string.0.as_concrete_TypeRef(),
        ))
    }

    /// returns the suggested size and the range of the string that fits.
    pub(crate) fn suggest_frame_size(
        &self,
        range: CFRange,
        constraints: CGSize,
    ) -> (CGSize, CFRange) {
        self.0
            .suggest_frame_size_with_constraints(range, std::ptr::null(), constraints)
    }

    pub(crate) fn create_frame(&self, range: CFRange, path: &CGPathRef) -> Frame {
        Frame(self.0.create_frame(range, path))
    }
}

impl Frame {
    pub(crate) fn get_lines(&self) -> CFArray<CTLine> {
        // we could just hold on to a Vec<CTLine> if we wanted?
        // this was written like this before we upstreamed changes to the core-text crate,
        // but those changes are more defensive, and do an extra allocation.
        // It might be simpler that way, though.
        unsafe { TCFType::wrap_under_get_rule(CTFrameGetLines(self.0.as_concrete_TypeRef())) }
    }

    pub(crate) fn get_line(&self, line_number: usize) -> Option<CTLine> {
        let idx: CFIndex = line_number.try_into().ok()?;
        let lines = self.get_lines();
        lines
            .get(idx)
            .map(|l| unsafe { TCFType::wrap_under_get_rule(l.as_concrete_TypeRef()) })
    }

    pub(crate) fn get_line_origins(&self, range: CFRange) -> Vec<CGPoint> {
        self.0.get_line_origins(range)
    }
}

impl<'a> Line<'a> {
    pub(crate) fn new(inner: &'a impl Deref<Target = CTLine>) -> Line<'a> {
        Line(Cow::Borrowed(inner.deref()))
    }

    pub(crate) fn get_string_range(&self) -> CFRange {
        self.0.get_string_range()
    }

    pub(crate) fn get_typographic_bounds(&self) -> TypographicBounds {
        self.0.get_typographic_bounds()
    }

    pub(crate) fn get_string_index_for_position(&self, position: CGPoint) -> CFIndex {
        self.0.get_string_index_for_position(position)
    }

    /// Return the 'primary' offset on the given line that the boundary of the
    /// character at the provided index.
    ///
    /// There is a 'secondary' offset that is not returned by the core-text crate,
    /// that is used for BiDi. We can worry about that when we worry about *that*.
    /// There are docs at:
    /// https://developer.apple.com/documentation/coretext/1509629-ctlinegetoffsetforstringindex
    pub(crate) fn get_offset_for_string_index(&self, index: CFIndex) -> CGFloat {
        self.0.get_string_offset_for_string_index(index)
    }
}

impl<'a> From<CTLine> for Line<'a> {
    fn from(src: CTLine) -> Line<'a> {
        Line(Cow::Owned(src))
    }
}

pub(crate) fn system_font(size: CGFloat) -> CTFont {
    unsafe {
        let font = CTFontCreateUIFontForLanguage(kCTFontSystemFontType, size, std::ptr::null());
        CTFont::wrap_under_create_rule(font)
    }
}

#[link(name = "CoreText", kind = "framework")]
extern "C" {
    fn CTFrameGetLines(frame: CTFrameRef) -> CFArrayRef;
    fn CTFontCreateUIFontForLanguage(
        font_type: CTFontUIFontType,
        size: CGFloat,
        language: CFStringRef,
    ) -> CTFontRef;
}
