//! Wrappers around CF/CT types, with nice interfaces.

use std::borrow::Cow;
use std::convert::TryInto;
use std::ffi::c_void;
use std::ops::Deref;

use core_foundation::{
    array::{CFArray, CFArrayRef, CFIndex},
    attributed_string::CFMutableAttributedString,
    base::{CFTypeID, TCFType},
    declare_TCFType,
    dictionary::{CFDictionary, CFDictionaryRef},
    impl_TCFType,
    number::CFNumber,
    string::{CFString, CFStringRef},
};
use core_foundation_sys::base::CFRange;
use core_graphics::{
    base::CGFloat,
    color::CGColor,
    data_provider::CGDataProvider,
    font::CGFont,
    geometry::{CGAffineTransform, CGPoint, CGRect, CGSize},
    path::CGPathRef,
};
use core_text::{
    font::{
        self, kCTFontSystemFontType, kCTFontUserFixedPitchFontType, CTFont, CTFontRef,
        CTFontUIFontType,
    },
    font_collection::{self, CTFontCollection, CTFontCollectionRef},
    font_descriptor::{self, CTFontDescriptor, CTFontDescriptorRef},
    frame::{CTFrame, CTFrameRef},
    framesetter::CTFramesetter,
    line::{CTLine, CTLineRef, TypographicBounds},
    string_attributes,
};
use foreign_types::ForeignType;
use unic_bidi::bidi_class::{BidiClass, BidiClassCategory};

use piet::kurbo::{Affine, Rect};
use piet::{Color, FontFamily, FontFamilyInner, TextAlignment};

#[derive(Clone)]
pub(crate) struct AttributedString {
    pub(crate) inner: CFMutableAttributedString,
    /// a guess as to text direction
    rtl: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct Framesetter(CTFramesetter);
#[derive(Debug, Clone)]
pub(crate) struct Frame(pub(crate) CTFrame);
#[derive(Debug, Clone)]
pub(crate) struct Line<'a>(pub(crate) Cow<'a, CTLine>);

#[derive(Debug, Clone)]
pub(crate) struct FontCollection(CTFontCollection);

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
    pub(crate) fn new(text: &str) -> Self {
        let mut inner = CFMutableAttributedString::new();
        let range = CFRange::init(0, 0);
        let cf_string = CFString::new(text);
        inner.replace_str(&cf_string, range);
        let rtl = first_strong_rtl(text);
        AttributedString { inner, rtl }
    }

    pub(crate) fn set_alignment(&mut self, alignment: TextAlignment) {
        let alignment = CTParagraphStyleSetting::alignment(alignment, self.rtl);
        let settings = [alignment];
        unsafe {
            let style = CTParagraphStyleCreate(settings.as_ptr(), 1);
            let style = CTParagraphStyle::wrap_under_create_rule(style);
            self.inner.set_attribute(
                self.range(),
                string_attributes::kCTParagraphStyleAttributeName,
                &style,
            );
        }
    }

    pub(crate) fn set_font(&mut self, range: CFRange, font: &CTFont) {
        unsafe {
            self.inner
                .set_attribute(range, string_attributes::kCTFontAttributeName, font);
        }
    }

    #[allow(non_upper_case_globals)]
    pub(crate) fn set_underline(&mut self, range: CFRange, underline: bool) {
        const kCTUnderlineStyleNone: i32 = 0x00;
        const kCTUnderlineStyleSingle: i32 = 0x01;

        let value = if underline {
            kCTUnderlineStyleSingle
        } else {
            kCTUnderlineStyleNone
        };
        unsafe {
            self.inner.set_attribute(
                range,
                string_attributes::kCTUnderlineStyleAttributeName,
                &CFNumber::from(value).as_CFType(),
            )
        }
    }

    #[allow(non_upper_case_globals)]
    pub(crate) fn set_strikethrough(&mut self, range: CFRange, strikethrough: bool) {
        // UnderlineStyle constants are also used for strikethrough styles
        const kCTUnderlineStyleNone: i32 = 0x00;
        const kCTUnderlineStyleSingle: i32 = 0x01;

        let value = if strikethrough {
            kCTUnderlineStyleSingle
        } else {
            kCTUnderlineStyleNone
        };
        unsafe {
            self.inner.set_attribute(
                range,
                string_attributes::kCTStrikethroughStyleAttributeName,
                &CFNumber::from(value).as_CFType(),
            )
        }
    }

    pub(crate) fn set_fg_color(&mut self, range: CFRange, color: &Color) {
        let (r, g, b, a) = color.as_rgba();
        let color = CGColor::rgb(r, g, b, a);
        unsafe {
            self.inner.set_attribute(
                range,
                string_attributes::kCTForegroundColorAttributeName,
                &color.as_CFType(),
            )
        }
    }

    pub(crate) fn range(&self) -> CFRange {
        CFRange::init(0, self.inner.char_len())
    }
}

impl Framesetter {
    pub(crate) fn new(attributed_string: &AttributedString) -> Self {
        Framesetter(CTFramesetter::new_with_attributed_string(
            attributed_string.inner.as_concrete_TypeRef(),
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

    pub(crate) fn get_image_bounds(&self) -> Rect {
        unsafe {
            let r = CTLineGetImageBounds(self.0.as_concrete_TypeRef(), std::ptr::null_mut());
            Rect::from_origin_size((r.origin.x, r.origin.y), (r.size.width, r.size.height))
        }
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

/// The apple system fonts can resolve to different concrete families at
/// different point sizes (SF Text vs. SF Displaykj,w)
pub(crate) fn ct_family_name(family: &FontFamily, size: f64) -> CFString {
    match &family.inner() {
        FontFamilyInner::Named(name) => CFString::new(name),
        other => system_font_family_name(other, size),
    }
}

/// Create a generic system font.
fn system_font_family_name(family: &FontFamilyInner, size: f64) -> CFString {
    let font = system_font_impl(family, size).unwrap_or_else(create_font_comma_never_fail_period);
    unsafe {
        let name = CTFontCopyName(font.as_concrete_TypeRef(), kCTFontFamilyNameKey);
        CFString::wrap_under_create_rule(name)
    }
}

fn system_font_impl(family: &FontFamilyInner, size: f64) -> Option<CTFont> {
    match family {
        FontFamilyInner::SansSerif | FontFamilyInner::SystemUi => {
            create_system_font(kCTFontSystemFontType, size)
        }
        //TODO: on 10.15 + we should be using new york here
        //FIXME: font::new_from_name actually never fails
        FontFamilyInner::Serif => font::new_from_name("Charter", size)
            .or_else(|_| font::new_from_name("Times", size))
            .or_else(|_| font::new_from_name("Times New Roman", size))
            .ok(),
        //TODO: on 10.15 we should be using SF-Mono here
        FontFamilyInner::Monospace => font::new_from_name("Menlo", size)
            .ok()
            .or_else(|| create_system_font(kCTFontUserFixedPitchFontType, size)),
        _ => panic!("system fontz only"),
    }
}

fn create_system_font(typ: CTFontUIFontType, size: f64) -> Option<CTFont> {
    unsafe {
        let font = CTFontCreateUIFontForLanguage(typ, size, std::ptr::null());
        if font.is_null() {
            None
        } else {
            Some(CTFont::wrap_under_create_rule(font))
        }
    }
}

fn create_font_comma_never_fail_period() -> CTFont {
    let empty_attributes = CFDictionary::from_CFType_pairs(&[]);
    let descriptor = font_descriptor::new_from_attributes(&empty_attributes);
    font::new_from_descriptor(&descriptor, 0.0)
}

pub(crate) fn add_font(font_data: &[u8]) -> Result<String, ()> {
    unsafe {
        let data = CGDataProvider::from_slice(font_data);
        let font_ref = CGFont::from_data_provider(data)?;
        let success = CTFontManagerRegisterGraphicsFont(font_ref.as_ptr(), std::ptr::null_mut());
        if success {
            let ct_font = font::new_from_CGFont(&font_ref, 0.0);
            Ok(ct_font.family_name())
        } else {
            Err(())
        }
    }
}

//TODO: this will probably be shared at some point?
/// A heurstic for text direction; returns `true` if, while enumerating characters
/// in this string, a character in the 'R' (strong right-to-left) category is
/// encountered before any character in the 'L' (strong left-to-right) category is.
///
/// See [Unicode technical report 9](https://unicode.org/reports/tr9/#Table_Bidirectional_Character_Types).
fn first_strong_rtl(text: &str) -> bool {
    text.chars()
        // an upper bound on how many chars we'll check
        .take(200)
        .map(BidiClass::of)
        .find(|c| c.category() == BidiClassCategory::Strong)
        .map(|c| c.is_rtl())
        .unwrap_or(false)
}

impl FontCollection {
    pub(crate) fn new_with_all_fonts() -> FontCollection {
        FontCollection(font_collection::create_for_all_families())
    }

    pub(crate) fn font_for_family_name(&mut self, name: &str) -> Option<FontFamily> {
        let name = CFString::from(name);
        unsafe {
            let array = CTFontCollectionCreateMatchingFontDescriptorsForFamily(
                self.0.as_concrete_TypeRef(),
                name.as_concrete_TypeRef(),
                std::ptr::null(),
            );

            if array.is_null() {
                None
            } else {
                let array = CFArray::<CTFontDescriptor>::wrap_under_create_rule(array);
                array
                    .get(0)
                    .map(|desc| FontFamily::new_unchecked(desc.family_name()))
            }
        }
    }
}

// the version of this in the coretext crate doesn't let you supply an affine
#[allow(clippy::many_single_char_names)]
pub(crate) fn make_font(desc: &CTFontDescriptor, pt_size: f64, affine: Affine) -> CTFont {
    let [a, b, c, d, e, f] = affine.as_coeffs();
    let affine = CGAffineTransform::new(a, b, c, d, e, f);
    unsafe {
        let font_ref = CTFontCreateWithFontDescriptor(
            desc.as_concrete_TypeRef(),
            pt_size as CGFloat,
            // ownership here isn't documented, which I presume to mean it is copied,
            // and we can pass a stack pointer.
            &affine as *const _,
        );
        CTFont::wrap_under_create_rule(font_ref)
    }
}

#[link(name = "CoreText", kind = "framework")]
extern "C" {
    static kCTFontFamilyNameKey: CFStringRef;

    pub static kCTFontVariationAxisIdentifierKey: CFStringRef;
    //static kCTFontVariationAxisMinimumValueKey: CFStringRef;
    //static kCTFontVariationAxisMaximumValueKey: CFStringRef;
    //static kCTFontVariationAxisDefaultValueKey: CFStringRef;
    //pub static kCTFontVariationAxisNameKey: CFStringRef;

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
    fn CTLineGetImageBounds(line: CTLineRef, ctx: *mut c_void) -> CGRect;
    fn CTFontCollectionCreateMatchingFontDescriptorsForFamily(
        collection: CTFontCollectionRef,
        family: CFStringRef,
        option: CFDictionaryRef,
    ) -> CFArrayRef;
    fn CTFontCopyName(font: CTFontRef, nameKey: CFStringRef) -> CFStringRef;
    fn CTFontCreateWithFontDescriptor(
        descriptor: CTFontDescriptorRef,
        size: CGFloat,
        matrix: *const CGAffineTransform,
    ) -> CTFontRef;
    fn CTFontManagerRegisterGraphicsFont(
        font: core_graphics::sys::CGFontRef,
        error: *mut c_void,
    ) -> bool;
}
