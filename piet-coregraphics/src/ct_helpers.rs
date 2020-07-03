//! Wrappers around CF/CT types, with nice interfaces.

use std::borrow::Cow;
use std::convert::TryInto;
use std::ffi::c_void;
use std::ops::Deref;

use core_foundation::{
    array::{CFArray, CFArrayRef, CFIndex},
    attributed_string::CFMutableAttributedString,
    base::{CFTypeID, TCFType},
    boolean::CFBoolean,
    declare_TCFType, impl_TCFType,
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

use piet::TextAlignment;

#[derive(Clone)]
pub(crate) struct AttributedString(pub(crate) CFMutableAttributedString);
#[derive(Debug, Clone)]
pub(crate) struct Framesetter(CTFramesetter);
#[derive(Debug, Clone)]
pub(crate) struct Frame(pub(crate) CTFrame);
#[derive(Debug, Clone)]
pub(crate) struct Line<'a>(pub(crate) Cow<'a, CTLine>);

pub enum __CTParagraphStyle {}
type CTParagraphStyleRef = *const __CTParagraphStyle;

declare_TCFType!(CTParagraphStyle, CTParagraphStyleRef);
impl_TCFType!(
    CTParagraphStyle,
    CTParagraphStyleRef,
    CTParagraphStyleGetTypeID
);

#[repr(u32)]
enum CTParagraphStyleSpecifier {
    Alignment = 0,
    //FirstLineHeadIndent = 1,
    //HeadIndent = 2,
    //TailIndent = 3,
    //TabStops = 4,
    //TabInterval = 5,
    //LineBreakMode = 6,
    // there are many more of these
}

#[repr(u8)]
enum CTTextAlignment {
    Left = 0,
    Right = 1,
    Center = 2,
    Justified = 3,
    Natural = 4,
}

#[repr(C)]
struct CTParagraphStyleSetting {
    spec: CTParagraphStyleSpecifier,
    value_size: usize,
    value: *const c_void,
}

impl CTParagraphStyleSetting {
    fn alignment(alignment: TextAlignment, is_rtl: bool) -> Self {
        static LEFT: CTTextAlignment = CTTextAlignment::Left;
        static RIGHT: CTTextAlignment = CTTextAlignment::Right;
        static CENTER: CTTextAlignment = CTTextAlignment::Center;
        static JUSTIFIED: CTTextAlignment = CTTextAlignment::Justified;
        static NATURAL: CTTextAlignment = CTTextAlignment::Natural;

        let alignment: *const CTTextAlignment = match alignment {
            TextAlignment::Start => &NATURAL,
            TextAlignment::End if is_rtl => &LEFT,
            TextAlignment::End => &RIGHT,
            TextAlignment::Center => &CENTER,
            TextAlignment::Justified => &JUSTIFIED,
        };

        CTParagraphStyleSetting {
            spec: CTParagraphStyleSpecifier::Alignment,
            value: alignment as *const c_void,
            value_size: std::mem::size_of::<CTTextAlignment>(),
        }
    }
}

impl AttributedString {
    pub(crate) fn new(text: &str, font: &CTFont, alignment: TextAlignment) -> Self {
        let mut string = CFMutableAttributedString::new();
        let range = CFRange::init(0, 0);
        let cf_string = CFString::new(text);

        string.replace_str(&cf_string, range);

        let str_len = string.char_len();
        let char_range = CFRange::init(0, str_len);

        unsafe {
            let lang = CFStringTokenizerCopyBestStringLanguage(
                cf_string.as_concrete_TypeRef(),
                char_range,
            );
            let is_rtl = if lang.is_null() {
                false
            } else {
                let lang = CFString::wrap_under_create_rule(lang);
                lang == "he" || lang == "ar"
            };
            let alignment = CTParagraphStyleSetting::alignment(alignment, is_rtl);

            let settings = [alignment];
            let style = CTParagraphStyleCreate(settings.as_ptr(), 1);
            let style = CTParagraphStyle::wrap_under_create_rule(style);

            string.set_attribute(char_range, string_attributes::kCTFontAttributeName, font);
            string.set_attribute::<CFBoolean>(
                char_range,
                string_attributes::kCTForegroundColorFromContextAttributeName,
                &CFBoolean::true_value(),
            );
            string.set_attribute(
                char_range,
                string_attributes::kCTParagraphStyleAttributeName,
                &style,
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

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringTokenizerCopyBestStringLanguage(string: CFStringRef, range: CFRange) -> CFStringRef;
}
#[link(name = "CoreText", kind = "framework")]
extern "C" {
    fn CTFrameGetLines(frame: CTFrameRef) -> CFArrayRef;
    fn CTFontCreateUIFontForLanguage(
        font_type: CTFontUIFontType,
        size: CGFloat,
        language: CFStringRef,
    ) -> CTFontRef;
    fn CTParagraphStyleGetTypeID() -> CFTypeID;
    fn CTParagraphStyleCreate(
        settings: *const CTParagraphStyleSetting,
        count: usize,
    ) -> CTParagraphStyleRef;
}
