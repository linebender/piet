//! Convenience wrappers for DirectWrite objects.

// TODO: get rid of this when we actually do use everything
#![allow(unused)]

use std::ptr::null_mut;

use winapi::shared::winerror::{HRESULT, SUCCEEDED};
use winapi::um::dwrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, IDWriteTextLayout,
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_NORMAL,
};
use winapi::Interface;

use wio::com::ComPtr;
use wio::wide::ToWide;

// TODO: minimize cut'n'paste; probably the best way to do this is
// unify with the crate error type
#[derive(Debug)]
pub enum Error {
    WinapiError(HRESULT),
}

pub struct DwriteFactory(ComPtr<IDWriteFactory>);

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
    format: &'a TextFormat,
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
    pub fn new(factory: &'a DwriteFactory, format: &'a TextFormat) -> TextLayoutBuilder<'a> {
        TextLayoutBuilder {
            factory,
            format,
            text: None,
            width: None,
            height: None,
        }
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
                self.format.0.as_raw(),
                width,
                height,
                &mut ptr,
            );
            wrap(hr, ptr, TextLayout)
        }
    }
}
