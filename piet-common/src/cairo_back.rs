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

use piet::{ImageBuf, ImageFormat};
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
pub type PietImage = CairoImage;

/// A struct that can be used to create bitmap render contexts.
///
/// In the case of Cairo, being a software renderer, no state is needed.
pub struct Device {
    // Since not all backends can support `Device: Sync`, make it non-Sync here to, for fewer
    // portability surprises.
    marker: std::marker::PhantomData<*const ()>,
}

unsafe impl Send for Device {}

/// A struct provides a `RenderContext` and then can have its bitmap extracted.
pub struct BitmapTarget<'a> {
    surface: ImageSurface,
    cr: Context,
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
        width: usize,
        height: usize,
        pix_scale: f64,
    ) -> Result<BitmapTarget, piet::Error> {
        let surface = ImageSurface::create(Format::ARgb32, width as i32, height as i32).unwrap();
        let cr = Context::new(&surface).unwrap();
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
        CairoRenderContext::new(&self.cr)
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
        let stride = self.surface.stride() as usize;
        let width = self.surface.width() as usize;
        let height = self.surface.height() as usize;
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
                assert!(!data_ptr.is_null(), "surface is finished");
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

    /// Get an in-memory pixel buffer from the bitmap.
    // Clippy complains about a to_xxx method taking &mut self. Semantically speaking, this is not
    // really a mutation, so we'll keep the name. Consider using interior mutability in the future.
    #[allow(clippy::wrong_self_convention)]
    pub fn to_image_buf(&mut self, fmt: ImageFormat) -> Result<ImageBuf, piet::Error> {
        let width = self.surface.width() as usize;
        let height = self.surface.height() as usize;
        let mut buf = vec![0; width * height * 4];
        self.copy_raw_pixels(fmt, &mut buf)?;
        Ok(ImageBuf::from_raw(buf, fmt, width, height))
    }

    /// Save bitmap to RGBA PNG file
    #[cfg(feature = "png")]
    pub fn save_to_file<P: AsRef<Path>>(mut self, path: P) -> Result<(), piet::Error> {
        let height = self.surface.height();
        let width = self.surface.width();
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
        Err(piet::Error::Unimplemented)
    }
}
