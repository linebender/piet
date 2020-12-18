// allows e.g. raw_data[dst_off + x * 4 + 2] = buf[src_off + x * 4 + 0];
#![allow(clippy::identity_op)]

//! Support for piet Skia back-end.

#[cfg(feature = "png")]
use png::{ColorType, Encoder};
#[cfg(feature = "png")]
use std::fs::File;
#[cfg(feature = "png")]
use std::io::BufWriter;
use std::marker::PhantomData;
use std::path::Path;

use piet::{ImageBuf, ImageFormat};
//#[doc(hidden)]
#[doc(hidden)]
pub use piet_skia::*;

/// The `RenderContext` for the Skia backend, which is selected.
pub type Piet<'a> = SkiaRenderContext<'a>;

/// The associated brush type for this backend.
///
/// This type matches `RenderContext::Brush`
pub type Brush = piet_skia::Brush;

/// The associated text factory for this backend.
///
/// This type matches `RenderContext::Text`
pub type PietText = SkiaText;

/// The associated text layout type for this backend.
///
/// This type matches `RenderContext::Text::TextLayout`
pub type PietTextLayout = SkiaTextLayout;

/// The associated text layout builder for this backend.
///
/// This type matches `RenderContext::Text::TextLayoutBuilder`
pub type PietTextLayoutBuilder = SkiaTextLayoutBuilder;

///// The associated image type for this backend.
/////
///// This type matches `RenderContext::Image`
//pub type Image = ImageSurface;
pub type Image = SkiaImage;

/// A struct that can be used to create bitmap render contexts.
///
/// In the case of Cairo, being a software renderer, no state is needed. // TODO comment wat?
pub struct Device {
    // Since not all backends can support `Device: Sync`, make it non-Sync here to, for fewer
    // portability surprises.
    marker: std::marker::PhantomData<*const ()>,
}

unsafe impl Send for Device {}

/// A struct provides a `RenderContext` and then can have its bitmap extracted.
pub struct BitmapTarget<'a> {
    //surface: ImageSurface,
    //cr: Context,
    phantom: PhantomData<&'a ()>,
}

impl Device {
    /// Create a new device.
    pub fn new() -> Result<Device, piet::Error> {
        Ok(Device {
            marker: std::marker::PhantomData,
        })
    }

    /// Create a new bitmap target.
    pub fn bitmap_target(
        &mut self,
        _width: usize,
        _height: usize,
        _pix_scale: f64,
    ) -> Result<BitmapTarget, piet::Error> {
        unimplemented!()
    }
}

impl<'a> BitmapTarget<'a> {
    /// Get a piet `RenderContext` for the bitmap.
    ///
    /// Note: caller is responsible for calling `finish` on the render
    /// context at the end of rendering.
    pub fn render_context(&mut self) -> SkiaRenderContext {
        unimplemented!()
    }

    /// Get raw RGBA pixels from the bitmap by copying them into `buf`. If all the pixels were
    /// copied, returns the number of bytes written. If `buf` wasn't big enough, returns an error
    /// and doesn't write anything.
    pub fn copy_raw_pixels(
        &mut self,
        _fmt: ImageFormat,
        _buf: &mut [u8],
    ) -> Result<usize, piet::Error> {
        unimplemented!();
    }

    /// Get an in-memory pixel buffer from the bitmap.
    // Clippy complains about a to_xxx method taking &mut self. Semantically speaking, this is not
    // really a mutation, so we'll keep the name. Consider using interior mutability in the future.
    #[allow(clippy::wrong_self_convention)]
    pub fn to_image_buf(&mut self, _fmt: ImageFormat) -> Result<ImageBuf, piet::Error> {
        unimplemented!();
    }

    /// Get raw RGBA pixels from the bitmap.
    #[deprecated(since = "0.2.0", note = "use to_image_buf")]
    pub fn into_raw_pixels(/*mut*/self, _fmt: ImageFormat) -> Result<Vec<u8>, piet::Error> {
        unimplemented!();
    }

    /// Save bitmap to RGBA PNG file
    #[cfg(feature = "png")]
    pub fn save_to_file<P: AsRef<Path>>(mut self, path: P) -> Result<(), piet::Error> {
        let height = self.surface.get_height();
        let width = self.surface.get_width();
        let image = self.to_image_buf(ImageFormat::RgbaPremul)?;
        let file = BufWriter::new(File::create(path).map_err(Into::<Box<_>>::into)?);
        let mut encoder = Encoder::new(file, width as u32, height as u32);
        encoder.set_color(ColorType::RGBA);
        encoder
            .write_header()
            .map_err(Into::<Box<_>>::into)?
            .write_image_data(image.raw_pixels())
            .map_err(Into::<Box<_>>::into)?;
        Ok(())
    }

    /// Stub for feature is missing
    #[cfg(not(feature = "png"))]
    pub fn save_to_file<P: AsRef<Path>>(self, _path: P) -> Result<(), piet::Error> {
        Err(piet::Error::MissingFeature)
    }
}
