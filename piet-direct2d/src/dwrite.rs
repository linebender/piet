// Copyright 2020 the Piet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Convenience wrappers for DirectWrite objects.

// TODO: get rid of this when we actually do use everything
#![allow(unused)]

use std::convert::TryInto;
use std::ffi::OsString;
use std::fmt::{Debug, Display, Formatter};
use std::mem::MaybeUninit;
use std::ptr::null_mut;
use std::sync::Arc;

use dwrote::FontCollection as DWFontCollection;
use winapi::Interface;
use winapi::shared::minwindef::{FALSE, TRUE};
use winapi::shared::ntdef::LOCALE_NAME_MAX_LENGTH;
use winapi::shared::winerror::{HRESULT, S_OK, SUCCEEDED};
use winapi::um::dwrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE,
    DWRITE_FONT_STYLE_ITALIC, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT,
    DWRITE_FONT_WEIGHT_NORMAL, DWRITE_HIT_TEST_METRICS, DWRITE_LINE_METRICS,
    DWRITE_OVERHANG_METRICS, DWRITE_READING_DIRECTION_RIGHT_TO_LEFT, DWRITE_TEXT_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_JUSTIFIED, DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING,
    DWRITE_TEXT_METRICS, DWRITE_TEXT_RANGE, DWriteCreateFactory, IDWriteFactory,
    IDWriteFontCollection, IDWriteFontFamily, IDWriteLocalizedStrings, IDWriteTextFormat,
    IDWriteTextLayout,
};
use winapi::um::unknwnbase::IUnknown;
use winapi::um::winnls::GetUserDefaultLocaleName;

use wio::com::ComPtr;
use wio::wide::{FromWide, ToWide};

use piet::kurbo::Insets;
use piet::{FontFamily as PietFontFamily, FontStyle, FontWeight, TextAlignment};

use crate::Brush;

/// "en-US" as null-terminated utf16.
const DEFAULT_LOCALE: &[u16] = &utf16_lit::utf16_null!("en-US");

/// The max layout constraint we use with dwrite.
///
/// On other platforms we use infinity, but on dwrite that contaminates
/// the values that we get back when calculating the image bounds.
// approximately the largest f32 without integer error
const MAX_LAYOUT_CONSTRAINT: f32 = 1.6e7;

// TODO: minimize cut'n'paste; probably the best way to do this is
// unify with the crate error type
pub enum Error {
    WinapiError(HRESULT),
}

/// This struct is public only to use for system integration in piet_common and druid-shell. It is not intended
/// that end-users directly use this struct.
#[derive(Clone)]
pub struct DwriteFactory(ComPtr<IDWriteFactory>);

// I couldn't find any documentation about using IDWriteFactory in a multi-threaded context.
// Hopefully, `Send` is a conservative enough assumption.
unsafe impl Send for DwriteFactory {}

#[derive(Clone)]
pub struct TextFormat(pub(crate) ComPtr<IDWriteTextFormat>);

#[derive(Clone)]
struct FontFamily(ComPtr<IDWriteFontFamily>);

pub struct FontCollection(ComPtr<IDWriteFontCollection>);

#[derive(Clone)]
pub struct TextLayout(ComPtr<IDWriteTextLayout>);

/// A range in a windows string, represented as a start position and a length.
#[derive(Debug, Clone, Copy)]
pub struct Utf16Range {
    pub start: usize,
    pub len: usize,
}

impl From<HRESULT> for Error {
    fn from(hr: HRESULT) -> Error {
        Error::WinapiError(hr)
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            Error::WinapiError(hr) => write!(f, "hresult {hr:x}"),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            Error::WinapiError(hr) => write!(f, "hresult {hr:x}"),
        }
    }
}

impl std::error::Error for Error {
    fn description(&self) -> &str {
        "winapi error"
    }
}

impl From<Error> for piet::Error {
    fn from(e: Error) -> piet::Error {
        piet::Error::BackendError(Box::new(e))
    }
}

unsafe fn wrap<T, U, F>(hr: HRESULT, ptr: *mut T, f: F) -> Result<U, Error>
where
    F: Fn(ComPtr<T>) -> U,
    T: Interface,
{
    if SUCCEEDED(hr) {
        Ok(f(ComPtr::from_raw(ptr)))
    } else {
        Err(hr.into())
    }
}

impl DwriteFactory {
    pub fn new() -> Result<DwriteFactory, Error> {
        unsafe {
            let mut ptr: *mut IDWriteFactory = null_mut();
            let hr = DWriteCreateFactory(
                DWRITE_FACTORY_TYPE_SHARED,
                &IDWriteFactory::uuidof(),
                &mut ptr as *mut _ as *mut _,
            );
            wrap(hr, ptr, DwriteFactory)
        }
    }

    pub fn get_raw(&self) -> *mut IDWriteFactory {
        self.0.as_raw()
    }

    pub(crate) fn system_font_collection(&self) -> Result<FontCollection, Error> {
        unsafe {
            let mut ptr = null_mut();
            let hr = self.0.GetSystemFontCollection(&mut ptr, 0);
            wrap(hr, ptr, FontCollection)
        }
    }

    /// Create from raw pointer
    ///
    /// # Safety
    /// TODO
    pub unsafe fn from_raw(raw: *mut IDWriteFactory) -> Self {
        Self(ComPtr::from_raw(raw))
    }
}

impl FontCollection {
    pub(crate) fn font_family(&self, name: &str) -> Option<PietFontFamily> {
        let wname = name.to_wide_null();
        let mut idx = u32::MAX;
        let mut exists = 0_i32;

        let family = unsafe {
            let hr = self.0.FindFamilyName(wname.as_ptr(), &mut idx, &mut exists);
            if SUCCEEDED(hr) && exists != 0 {
                let mut family = null_mut();
                let hr = self.0.GetFontFamily(idx, &mut family);
                wrap(hr, family, FontFamily).ok()
            } else {
                eprintln!(
                    "failed to find family name {}: err {} not_found: {}",
                    name, hr, !exists
                );
                None
            }
        }?;

        family.family_name().ok()
    }
}

impl FontFamily {
    // this monster is taken right out of the docs :/
    /// Returns the localized name of this family.
    fn family_name(&self) -> Result<PietFontFamily, Error> {
        unsafe {
            let mut names = null_mut();
            let hr = self.0.GetFamilyNames(&mut names);
            if !SUCCEEDED(hr) {
                return Err(hr.into());
            }

            let names: ComPtr<IDWriteLocalizedStrings> = ComPtr::from_raw(names);

            let mut index = 0_u32;
            let mut exists = 0_i32;
            let mut locale_name = [0_u16; LOCALE_NAME_MAX_LENGTH];

            let success =
                GetUserDefaultLocaleName(locale_name.as_mut_ptr(), LOCALE_NAME_MAX_LENGTH as i32);
            let mut hr = if SUCCEEDED(success) {
                names.FindLocaleName(locale_name.as_ptr(), &mut index, &mut exists)
            } else {
                // we reuse the previous success; we want  to run the next block
                // both if the previous failed, or if it just doesn't find anything
                hr
            };
            if !SUCCEEDED(hr) || exists == 0 {
                hr = names.FindLocaleName(DEFAULT_LOCALE.as_ptr(), &mut index, &mut exists);
            }

            if !SUCCEEDED(hr) {
                return Err(hr.into());
            }

            // if locale doesn't exist, just choose the first
            if exists == 0 {
                index = 0;
            }

            let mut length = 0_u32;
            let hr = names.GetStringLength(index, &mut length);

            if !SUCCEEDED(hr) {
                return Err(hr.into());
            }

            let mut wide_name: Vec<u16> = Vec::with_capacity(length as usize + 1);
            let hr = names.GetString(index, wide_name.as_mut_ptr(), length + 1);
            if SUCCEEDED(hr) {
                wide_name.set_len(length as usize + 1);
                let name = OsString::from_wide(&wide_name)
                    .into_string()
                    .unwrap_or_else(|err| err.to_string_lossy().into_owned());

                Ok(PietFontFamily::new_unchecked(name))
            } else {
                Err(hr.into())
            }
        }
    }
}

impl TextFormat {
    pub(crate) fn new(
        factory: &DwriteFactory,
        family: impl AsRef<[u16]>,
        size: f32,
        rtl: bool,
    ) -> Result<TextFormat, Error> {
        let family = family.as_ref();

        unsafe {
            let mut ptr = null_mut();
            let hr = factory.0.CreateTextFormat(
                family.as_ptr(),
                null_mut(), // collection
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                //TODO: this should be the user's locale? It will influence font fallback behaviour?
                DEFAULT_LOCALE.as_ptr(),
                &mut ptr,
            );

            let r = wrap(hr, ptr, TextFormat)?;
            if rtl {
                r.0.SetReadingDirection(DWRITE_READING_DIRECTION_RIGHT_TO_LEFT);
            }
            Ok(r)
        }
    }
}

#[allow(overflowing_literals)]
#[allow(clippy::unreadable_literal)]
const E_NOT_SUFFICIENT_BUFFER: HRESULT = 0x8007007A;

impl Utf16Range {
    pub fn new(start: usize, len: usize) -> Self {
        Utf16Range { start, len }
    }
}

impl From<Utf16Range> for DWRITE_TEXT_RANGE {
    fn from(src: Utf16Range) -> DWRITE_TEXT_RANGE {
        let Utf16Range { start, len } = src;
        DWRITE_TEXT_RANGE {
            startPosition: start.try_into().unwrap(),
            length: len.try_into().unwrap(),
        }
    }
}

impl TextLayout {
    pub(crate) fn new(
        dwrite: &DwriteFactory,
        format: TextFormat,
        width: f32,
        text: &[u16],
    ) -> Result<Self, Error> {
        let len: u32 = text.len().try_into().unwrap();
        // d2d doesn't handle infinity very well
        let width = if !width.is_finite() {
            MAX_LAYOUT_CONSTRAINT
        } else {
            width
        };

        unsafe {
            let mut ptr = null_mut();
            let hr = dwrite.0.CreateTextLayout(
                text.as_ptr(),
                len,
                format.0.as_raw(),
                width,
                MAX_LAYOUT_CONSTRAINT,
                &mut ptr,
            );
            wrap(hr, ptr, TextLayout)
        }
    }

    /// Set the alignment for this entire layout.
    pub(crate) fn set_alignment(&mut self, alignment: TextAlignment) {
        let alignment = match alignment {
            TextAlignment::Start => DWRITE_TEXT_ALIGNMENT_LEADING,
            TextAlignment::End => DWRITE_TEXT_ALIGNMENT_TRAILING,
            TextAlignment::Center => DWRITE_TEXT_ALIGNMENT_CENTER,
            TextAlignment::Justified => DWRITE_TEXT_ALIGNMENT_JUSTIFIED,
        };

        unsafe {
            self.0.SetTextAlignment(alignment);
        }
    }

    /// Set the weight for a range of this layout. `start` and `len` are in utf16.
    pub(crate) fn set_weight(&mut self, range: Utf16Range, weight: FontWeight) {
        let weight = weight.to_raw() as DWRITE_FONT_WEIGHT;
        unsafe {
            self.0.SetFontWeight(weight, range.into());
        }
    }

    pub(crate) fn set_font_family(&mut self, range: Utf16Range, family: &str) {
        let wide_name = family.to_wide_null();
        unsafe {
            self.0.SetFontFamilyName(wide_name.as_ptr(), range.into());
        }
    }

    pub(crate) fn set_font_collection(&mut self, range: Utf16Range, collection: &DWFontCollection) {
        unsafe {
            self.0.SetFontCollection(collection.as_ptr(), range.into());
        }
    }

    pub(crate) fn set_style(&mut self, range: Utf16Range, style: FontStyle) {
        let val = match style {
            FontStyle::Italic => DWRITE_FONT_STYLE_ITALIC,
            FontStyle::Regular => DWRITE_FONT_STYLE_NORMAL,
        };
        unsafe {
            self.0.SetFontStyle(val, range.into());
        }
    }

    pub(crate) fn set_underline(&mut self, range: Utf16Range, flag: bool) {
        let flag = if flag { TRUE } else { FALSE };
        unsafe {
            self.0.SetUnderline(flag, range.into());
        }
    }

    pub(crate) fn set_strikethrough(&mut self, range: Utf16Range, flag: bool) {
        let flag = if flag { TRUE } else { FALSE };
        unsafe {
            self.0.SetStrikethrough(flag, range.into());
        }
    }

    pub(crate) fn set_size(&mut self, range: Utf16Range, size: f32) {
        unsafe {
            self.0.SetFontSize(size, range.into());
        }
    }

    pub(crate) fn set_foreground_brush(&mut self, range: Utf16Range, brush: Brush) {
        unsafe {
            self.0
                .SetDrawingEffect(brush.as_raw() as *mut IUnknown, range.into());
        }
    }

    /// Get line metrics, storing them in the provided buffer.
    ///
    /// Note: this isn't necessarily the lowest level wrapping, as it requires
    /// an allocation for the buffer. But it's pretty ergonomic.
    pub fn get_line_metrics(&self, buf: &mut Vec<DWRITE_LINE_METRICS>) {
        let cap = buf.capacity().min(0xffff_ffff) as u32;
        unsafe {
            let mut actual_count = 0;
            let mut hr = self
                .0
                .GetLineMetrics(buf.as_mut_ptr(), cap, &mut actual_count);
            if hr == E_NOT_SUFFICIENT_BUFFER {
                buf.reserve(actual_count as usize - buf.len());
                hr = self
                    .0
                    .GetLineMetrics(buf.as_mut_ptr(), actual_count, &mut actual_count);
            }
            if SUCCEEDED(hr) {
                buf.set_len(actual_count as usize);
            } else {
                buf.set_len(0);
            }
        }
    }

    pub fn get_raw(&self) -> *mut IDWriteTextLayout {
        self.0.as_raw()
    }

    pub fn get_metrics(&self) -> DWRITE_TEXT_METRICS {
        unsafe {
            let mut result = std::mem::zeroed();
            self.0.GetMetrics(&mut result);
            result
        }
    }

    /// Return the DWRITE_OVERHANG_METRICS, converted to an `Insets` struct.
    ///
    /// The 'right' and 'bottom' values of this struct are relative to the *layout*
    /// width and height; that is, the width and height constraints used to create
    /// the layout, not the actual size of the generated layout.
    pub fn get_overhang_metrics(&self) -> Insets {
        unsafe {
            let mut result = std::mem::zeroed();
            // returning all 0s on failure feels okay?
            let _ = self.0.GetOverhangMetrics(&mut result);
            let DWRITE_OVERHANG_METRICS {
                left,
                top,
                right,
                bottom,
            } = result;
            Insets::new(left as f64, top as f64, right as f64, bottom as f64)
        }
    }

    pub fn set_max_width(&mut self, max_width: f64) -> Result<(), Error> {
        // infinity produces nonsense values for the inking rect on d2d
        let max_width = if !max_width.is_finite() {
            MAX_LAYOUT_CONSTRAINT
        } else {
            max_width as f32
        };

        unsafe {
            let hr = self.0.SetMaxWidth(max_width);

            if SUCCEEDED(hr) {
                Ok(())
            } else {
                Err(hr.into())
            }
        }
    }

    pub fn hit_test_point(&self, point_x: f32, point_y: f32) -> HitTestPoint {
        unsafe {
            let mut trail = 0;
            let mut inside = 0;
            let mut metrics = MaybeUninit::uninit();
            self.0.HitTestPoint(
                point_x,
                point_y,
                &mut trail,
                &mut inside,
                metrics.as_mut_ptr(),
            );

            HitTestPoint {
                metrics: metrics.assume_init().into(),
                is_inside: inside != 0,
                is_trailing_hit: trail != 0,
            }
        }
    }

    pub fn hit_test_text_position(
        &self,
        position: u32,
        trailing: bool,
    ) -> Option<HitTestTextPosition> {
        let trailing = trailing as i32;
        unsafe {
            let (mut x, mut y) = (0.0, 0.0);
            let mut metrics = std::mem::zeroed();
            let res = self
                .0
                .HitTestTextPosition(position, trailing, &mut x, &mut y, &mut metrics);
            if res != S_OK {
                return None;
            }

            Some(HitTestTextPosition {
                metrics: metrics.into(),
                point_x: x,
                point_y: y,
            })
        }
    }
}

#[derive(Copy, Clone)]
/// Results from calling `hit_test_point` on a TextLayout.
pub struct HitTestPoint {
    /// The output geometry fully enclosing the hit-test location. When is_inside is set to false,
    /// this structure represents the geometry enclosing the edge closest to the hit-test location.
    pub metrics: HitTestMetrics,
    /// An output flag that indicates whether the hit-test location is inside the text string. When
    /// false, the position nearest the text's edge is returned.
    pub is_inside: bool,
    /// An output flag that indicates whether the hit-test location is at the leading or the
    /// trailing side of the character. When is_inside is set to false, this value is set according
    /// to the output hitTestMetrics->textPosition value to represent the edge closest to the
    /// hit-test location.
    pub is_trailing_hit: bool,
}

#[derive(Copy, Clone)]
/// Results from calling `hit_test_text_position` on a TextLayout.
pub struct HitTestTextPosition {
    /// The output pixel location X, relative to the top-left location of the layout box.
    pub point_x: f32,
    /// The output pixel location Y, relative to the top-left location of the layout box.
    pub point_y: f32,

    /// The output geometry fully enclosing the specified text position.
    pub metrics: HitTestMetrics,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
/// Describes the region obtained by a hit test.
pub struct HitTestMetrics {
    /// The first text position within the hit region.
    pub text_position: u32,
    /// The number of text positions within the hit region.
    pub length: u32,
    /// The x-coordinate of the upper-left corner of the hit region.
    pub left: f32,
    /// The y-coordinate of the upper-left corner of the hit region.
    pub top: f32,
    /// The width of the hit region.
    pub width: f32,
    /// The height of the hit region.
    pub height: f32,
    /// The BIDI level of the text positions within the hit region.
    pub bidi_level: u32,
    /// Non-zero if the hit region contains text; otherwise, `0`.
    pub is_text: bool,
    /// Non-zero if the text range is trimmed; otherwise, `0`.
    pub is_trimmed: bool,
}

impl From<DWRITE_HIT_TEST_METRICS> for HitTestMetrics {
    fn from(metrics: DWRITE_HIT_TEST_METRICS) -> Self {
        HitTestMetrics {
            text_position: metrics.textPosition,
            length: metrics.length,
            left: metrics.left,
            top: metrics.top,
            width: metrics.width,
            height: metrics.height,
            bidi_level: metrics.bidiLevel,
            is_text: metrics.isText != 0,
            is_trimmed: metrics.isTrimmed != 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_names() {
        let factory = DwriteFactory::new().unwrap();
        let fonts = factory.system_font_collection().unwrap();
        assert!(fonts.font_family("serif").is_none());
        assert!(fonts.font_family("arial").is_some());
        assert!(fonts.font_family("Arial").is_some());
        assert!(fonts.font_family("Times New Roman").is_some());
    }

    #[test]
    fn default_locale() {
        assert_eq!("en-US".to_wide_null().as_slice(), DEFAULT_LOCALE);
    }
}
