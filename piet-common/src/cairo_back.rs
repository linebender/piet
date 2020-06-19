// allows e.g. raw_data[dst_off + x * 4 + 2] = buf[src_off + x * 4 + 0];
#![allow(clippy::identity_op)]

//! Support for piet Cairo back-end.

use cairo::{Context, Format, ImageSurface};
#[cfg(feature = "png")]
use png::{ColorType, Encoder};
#[cfg(feature = "png")]
use std::fs::File;
#[cfg(feature = "png")]
use std::io::BufWriter;
use std::marker::PhantomData;
use std::path::Path;

use piet::ImageFormat;
#[doc(hidden)]
pub use piet_cairo::*;

/// The `RenderContext` for the Cairo backend, which is selected.
pub type Piet<'a> = CairoRenderContext<'a>;

/// The associated brush type for this backend.
///
/// This type matches `RenderContext::Brush`
pub type Brush = piet_cairo::Brush;

/// The associated text factory for this backend.
///
/// This type matches `RenderContext::Text`
pub type PietText = CairoText;

/// The associated font type for this backend.
///
/// This type matches `RenderContext::Text::Font`
pub type PietFont = CairoFont;

/// The associated font builder for this backend.
///
/// This type matches `RenderContext::Text::FontBuilder`
pub type PietFontBuilder = CairoFontBuilder;

/// The associated text layout type for this backend.
///
/// This type matches `RenderContext::Text::TextLayout`
pub type PietTextLayout = CairoTextLayout;

/// The associated text layout builder for this backend.
///
/// This type matches `RenderContext::Text::TextLayoutBuilder`
pub type PietTextLayoutBuilder = CairoTextLayoutBuilder;

/// The associated image type for this backend.
///
/// This type matches `RenderContext::Image`
pub type Image = ImageSurface;

/// A struct that can be used to create bitmap render contexts.
///
/// In the case of Cairo, being a software renderer, no state is needed.
pub struct Device;

/// A struct provides a `RenderContext` and then can have its bitmap extracted.
pub struct BitmapTarget<'a> {
    surface: ImageSurface,
    cr: Context,
    phantom: PhantomData<&'a ()>,
}

impl Device {
    /// Create a new device.
    pub fn new() -> Result<Device, piet::Error> {
        Ok(Device)
    }

    /// Create a new bitmap target.
    pub fn bitmap_target(
        &mut self,
        width: usize,
        height: usize,
        pix_scale: f64,
    ) -> Result<BitmapTarget, piet::Error> {
        let surface = ImageSurface::create(Format::ARgb32, width as i32, height as i32).unwrap();
        let cr = Context::new(&surface);
        cr.scale(pix_scale, pix_scale);
        let phantom = Default::default();
        Ok(BitmapTarget {
            surface,
            cr,
            phantom,
        })
    }
}

impl<'a> BitmapTarget<'a> {
    /// Get a piet `RenderContext` for the bitmap.
    ///
    /// Note: caller is responsible for calling `finish` on the render
    /// context at the end of rendering.
    pub fn render_context(&mut self) -> CairoRenderContext {
        CairoRenderContext::new(&mut self.cr)
    }

    /// Get raw RGBA pixels from the bitmap by copying them into `buf`. If all the pixels were
    /// copied, returns the number of bytes written. If `buf` wasn't big enough, returns an error
    /// and doesn't write anything.
    pub fn copy_raw_pixels(
        &mut self,
        fmt: ImageFormat,
        buf: &mut [u8],
    ) -> Result<usize, piet::Error> {
        // TODO: convert other formats.
        if fmt != ImageFormat::RgbaPremul {
            return Err(piet::Error::NotSupported);
        }
        self.surface.flush();
        let stride = self.surface.get_stride() as usize;
        let width = self.surface.get_width() as usize;
        let height = self.surface.get_height() as usize;
        let size = width * height * 4;
        if buf.len() < size {
            return Err(piet::Error::InvalidInput);
        }
        unsafe {
            // Cairo's rust wrapper has extra safety checks that we want to avoid: it won't let us
            // get the data from an ImageSurface that's still referenced by a context. The C docs
            // don't seem to think that's a problem, as long as we call flush (which we already
            // did), and promise not to mutate anything.
            // https://www.cairographics.org/manual/cairo-Image-Surfaces.html#cairo-image-surface-get-data
            //
            // TODO: we can simplify this once cairo makes a release containing
            // https://github.com/gtk-rs/cairo/pull/330
            let data_len = height.saturating_sub(1) * stride + width * 4;
            let data = {
                let data_ptr = cairo_sys::cairo_image_surface_get_data(self.surface.to_raw_none());
                if data_ptr.is_null() {
                    let err = cairo::BorrowError::from(cairo::Status::SurfaceFinished);
                    return Err((Box::new(err) as Box<dyn std::error::Error>).into());
                }
                std::slice::from_raw_parts(data_ptr, data_len)
            };

            // A sanity check for all the unsafe indexing that follows.
            assert!(data.get(data_len - 1).is_some());
            assert!(buf.get(size - 1).is_some());

            for y in 0..height {
                let src_off = y * stride;
                let dst_off = y * width * 4;
                for x in 0..width {
                    // These unchecked indexes allow the autovectorizer to shine.
                    // Note that dst_off maxes out at (height - 1) * width * 4, and so
                    // dst_off + x * 4 + 3 maxes out at height * width * 4 - 1, which is size - 1.
                    // Also, src_off maxes out at (height - 1) * stride, and so
                    // src_off + x * 4 + 3 maxes out at (height - 1) * stride + width * 4 - 1,
                    // which is data_len - 1.
                    *buf.get_unchecked_mut(dst_off + x * 4 + 0) =
                        *data.get_unchecked(src_off + x * 4 + 2);
                    *buf.get_unchecked_mut(dst_off + x * 4 + 1) =
                        *data.get_unchecked(src_off + x * 4 + 1);
                    *buf.get_unchecked_mut(dst_off + x * 4 + 2) =
                        *data.get_unchecked(src_off + x * 4 + 0);
                    *buf.get_unchecked_mut(dst_off + x * 4 + 3) =
                        *data.get_unchecked(src_off + x * 4 + 3);
                }
            }
        }
        Ok(size)
    }

    /// Get raw RGBA pixels from the bitmap.
    pub fn raw_pixels(&mut self, fmt: ImageFormat) -> Result<Vec<u8>, piet::Error> {
        let width = self.surface.get_width() as usize;
        let height = self.surface.get_height() as usize;
        let mut buf = vec![0; width * height * 4];
        self.copy_raw_pixels(fmt, &mut buf)?;
        Ok(buf)
    }

    /// Get raw RGBA pixels from the bitmap.
    #[deprecated(since = "0.2.0", note = "use raw_pixels")]
    pub fn into_raw_pixels(mut self, fmt: ImageFormat) -> Result<Vec<u8>, piet::Error> {
        self.raw_pixels(fmt)
    }

    /// Save bitmap to RGBA PNG file
    #[cfg(feature = "png")]
    pub fn save_to_file<P: AsRef<Path>>(mut self, path: P) -> Result<(), piet::Error> {
        let height = self.surface.get_height();
        let width = self.surface.get_width();
        let image = self.raw_pixels(ImageFormat::RgbaPremul)?;
        let file = BufWriter::new(File::create(path).map_err(Into::<Box<_>>::into)?);
        let mut encoder = Encoder::new(file, width as u32, height as u32);
        encoder.set_color(ColorType::RGBA);
        encoder
            .write_header()
            .map_err(Into::<Box<_>>::into)?
            .write_image_data(&image)
            .map_err(Into::<Box<_>>::into)?;
        Ok(())
    }

    /// Stub for feature is missing
    #[cfg(not(feature = "png"))]
    pub fn save_to_file<P: AsRef<Path>>(self, _path: P) -> Result<(), piet::Error> {
        Err(piet::Error::MissingFeature)
    }
}
