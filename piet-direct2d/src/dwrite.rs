//! Convenience wrappers for DirectWrite objects.

// TODO: get rid of this when we actually do use everything
#![allow(unused)]

use std::fmt::{Debug, Display, Formatter};
use std::ptr::null_mut;

use winapi::shared::winerror::{HRESULT, SUCCEEDED};
use winapi::um::dwrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, IDWriteTextLayout,
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_NORMAL, DWRITE_LINE_METRICS, DWRITE_TEXT_METRICS,
};
use winapi::Interface;

use wio::com::ComPtr;
use wio::wide::ToWide;

use piet::{new_error, ErrorKind};

// TODO: minimize cut'n'paste; probably the best way to do this is
// unify with the crate error type
pub enum Error {
    WinapiError(HRESULT),
}

pub struct DwriteFactory(ComPtr<IDWriteFactory>);

#[derive(Clone)]
pub struct TextFormat(ComPtr<IDWriteTextFormat>);

/// A builder for creating new `TextFormat` objects.
///
/// TODO: provide lots more capability.
pub struct TextFormatBuilder<'a> {
    factory: &'a DwriteFactory,
    size: Option<f32>,
    family: Option<&'a str>,
}

pub struct TextLayoutBuilder<'a> {
    factory: &'a DwriteFactory,
    format: Option<TextFormat>,
    text: Option<Vec<u16>>,
    width: Option<f32>,
    height: Option<f32>,
}

pub struct TextLayout(ComPtr<IDWriteTextLayout>);

impl From<HRESULT> for Error {
    fn from(hr: HRESULT) -> Error {
        Error::WinapiError(hr)
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            Error::WinapiError(hr) => write!(f, "hresult {:x}", hr),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            Error::WinapiError(hr) => write!(f, "hresult {:x}", hr),
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
        new_error(ErrorKind::BackendError(Box::new(e)))
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
}

impl<'a> TextFormatBuilder<'a> {
    pub fn new(factory: &'a DwriteFactory) -> TextFormatBuilder<'a> {
        TextFormatBuilder {
            factory,
            size: None,
            family: None,
        }
    }

    pub fn size(mut self, size: f32) -> TextFormatBuilder<'a> {
        self.size = Some(size);
        self
    }

    pub fn family(mut self, family: &'a str) -> TextFormatBuilder<'a> {
        self.family = Some(family);
        self
    }

    pub fn build(self) -> Result<TextFormat, Error> {
        let family = self
            .family
            .expect("`family` must be specified")
            .to_wide_null();
        let size = self.size.expect("`size` must be specified");
        let locale = "en-US".to_wide_null();
        unsafe {
            let mut ptr = null_mut();
            let hr = self.factory.0.CreateTextFormat(
                family.as_ptr(),
                null_mut(), // collection
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                locale.as_ptr(),
                &mut ptr,
            );
            wrap(hr, ptr, TextFormat)
        }
    }
}

impl<'a> TextLayoutBuilder<'a> {
    pub fn new(factory: &'a DwriteFactory) -> TextLayoutBuilder<'a> {
        TextLayoutBuilder {
            factory,
            format: None,
            text: None,
            width: None,
            height: None,
        }
    }

    pub fn format(mut self, format: &TextFormat) -> TextLayoutBuilder<'a> {
        // The fact we clone here is annoying, but it gets us out of
        // otherwise annoying lifetime issues.
        self.format = Some(format.clone());
        self
    }

    pub fn text(mut self, text: &str) -> TextLayoutBuilder<'a> {
        self.text = Some(text.to_wide());
        self
    }

    pub fn width(mut self, width: f32) -> TextLayoutBuilder<'a> {
        self.width = Some(width);
        self
    }

    pub fn height(mut self, height: f32) -> TextLayoutBuilder<'a> {
        self.height = Some(height);
        self
    }

    pub fn build(self) -> Result<TextLayout, Error> {
        let format = self.format.expect("`format` must be specified");
        let text = self.text.expect("`text` must be specified");
        let len = text.len();
        assert!(len <= 0xffff_ffff);
        let width = self.width.expect("`width` must be specified");
        let height = self.height.expect("`height` must be specified");
        unsafe {
            let mut ptr = null_mut();
            let hr = self.factory.0.CreateTextLayout(
                text.as_ptr(),
                len as u32,
                format.0.as_raw(),
                width,
                height,
                &mut ptr,
            );
            wrap(hr, ptr, TextLayout)
        }
    }
}

#[allow(overflowing_literals)]
const E_NOT_SUFFICIENT_BUFFER: HRESULT = 0x8007007A;

impl TextLayout {
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
                buf.reserve((actual_count - cap) as usize);
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
}
