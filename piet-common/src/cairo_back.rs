// Copyright 2019 the Piet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

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

#[cfg(feature = "png")]
use piet::util;
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
    ///
    /// Note: caller is responsible for making sure the requested `ImageFormat` is supported.
    pub fn copy_raw_pixels(
        &mut self,
        fmt: ImageFormat,
        buf: &mut [u8],
    ) -> Result<usize, piet::Error> {
        // TODO: convert other formats.
        if fmt != ImageFormat::RgbaPremul {
            return Err(piet::Error::NotSupported);
        }

        // Ensure the surface is ready to be read from.
        self.surface.flush();

        // Get the surface dimensions.
        let stride = self.surface.stride() as usize;
        let width = self.surface.width() as usize;
        let height = self.surface.height() as usize;

        // Get the expected destination and source buffer lengths.
        let dst_len = width * height * 4;
        let src_len = height.saturating_sub(1) * stride + width * 4;

        // Validate that the buffer we will be writing into has at least the
        // required length to be filled.
        if buf.len() < dst_len {
            return Err(piet::Error::InvalidInput);
        }

        // Copy the the image surface data to the destination buffer.
        self.surface
            .with_data(|src| {
                // Sanity check for all the unsafe indexing that follows.
                debug_assert!(src.len() >= src_len);
                debug_assert!(buf.len() >= dst_len);

                unsafe {
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
                                *src.get_unchecked(src_off + x * 4 + 2);
                            *buf.get_unchecked_mut(dst_off + x * 4 + 1) =
                                *src.get_unchecked(src_off + x * 4 + 1);
                            *buf.get_unchecked_mut(dst_off + x * 4 + 2) =
                                *src.get_unchecked(src_off + x * 4 + 0);
                            *buf.get_unchecked_mut(dst_off + x * 4 + 3) =
                                *src.get_unchecked(src_off + x * 4 + 3);
                        }
                    }
                }
            })
            .map_err(|err| piet::Error::BackendError(Box::new(err)))?;

        Ok(dst_len)
    }

    /// Get an in-memory pixel buffer from the bitmap.
    ///
    /// Note: caller is responsible for making sure the requested `ImageFormat` is supported.
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
        let width = self.surface.width() as usize;
        let height = self.surface.height() as usize;
        let mut data = vec![0; width * height * 4];
        self.copy_raw_pixels(ImageFormat::RgbaPremul, &mut data)?;
        util::unpremultiply_rgba(&mut data);
        let file = BufWriter::new(File::create(path).map_err(Into::<Box<_>>::into)?);
        let mut encoder = Encoder::new(file, width as u32, height as u32);
        encoder.set_color(ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .map_err(Into::<Box<_>>::into)?
            .write_image_data(&data)
            .map_err(Into::<Box<_>>::into)?;
        Ok(())
    }

    /// Stub for feature is missing
    #[cfg(not(feature = "png"))]
    pub fn save_to_file<P: AsRef<Path>>(self, _path: P) -> Result<(), piet::Error> {
        Err(piet::Error::Unimplemented)
    }
}
