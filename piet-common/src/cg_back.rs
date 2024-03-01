// Copyright 2020 the Piet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

// allows e.g. raw_data[dst_off + x * 4 + 2] = buf[src_off + x * 4 + 0];
#![allow(clippy::identity_op)]

//! Support for piet CoreGraphics back-end.

use std::marker::PhantomData;
use std::path::Path;
#[cfg(feature = "png")]
use std::{fs::File, io::BufWriter};

use core_graphics::{color_space::CGColorSpace, context::CGContext};
#[cfg(feature = "png")]
use png::{ColorType, Encoder};

#[cfg(feature = "png")]
use piet::util;
use piet::{Error, ImageBuf, ImageFormat};
#[doc(hidden)]
pub use piet_coregraphics::*;

/// The `RenderContext` for the CoreGraphics backend, which is selected.
pub type Piet<'a> = CoreGraphicsContext<'a>;

/// The associated brush type for this backend.
///
/// This type matches `RenderContext::Brush`
pub type Brush = piet_coregraphics::Brush;

/// The associated text factory for this backend.
///
/// This type matches `RenderContext::Text`
pub type PietText = CoreGraphicsText;

/// The associated text layout type for this backend.
///
/// This type matches `RenderContext::Text::TextLayout`
pub type PietTextLayout = CoreGraphicsTextLayout;

/// The associated text layout builder for this backend.
///
/// This type matches `RenderContext::Text::TextLayoutBuilder`
pub type PietTextLayoutBuilder = CoreGraphicsTextLayoutBuilder;

/// The associated image type for this backend.
///
/// This type matches `RenderContext::Image`
pub type PietImage = CoreGraphicsImage;

/// A struct that can be used to create bitmap render contexts.
pub struct Device {
    // Since not all backends can support `Device: Sync`, make it non-Sync here to, for fewer
    // portability surprises.
    marker: std::marker::PhantomData<*const ()>,
}

unsafe impl Send for Device {}

/// A struct provides a `RenderContext` and then can have its bitmap extracted.
pub struct BitmapTarget<'a> {
    ctx: CGContext,
    height: f64,
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
        let ctx = CGContext::create_bitmap_context(
            None,
            width,
            height,
            8,
            0,
            &CGColorSpace::create_device_rgb(),
            core_graphics::base::kCGImageAlphaPremultipliedLast,
        );
        ctx.scale(pix_scale, pix_scale);
        let height = height as f64 * pix_scale.recip();
        Ok(BitmapTarget {
            ctx,
            height,
            phantom: PhantomData,
        })
    }
}

impl<'a> BitmapTarget<'a> {
    /// Get a piet `RenderContext` for the bitmap.
    ///
    /// Note: caller is responsible for calling `finish` on the render
    /// context at the end of rendering.
    pub fn render_context(&mut self) -> CoreGraphicsContext {
        CoreGraphicsContext::new_y_up(&mut self.ctx, self.height, None)
    }

    /// Get an in-memory pixel buffer from the bitmap.
    ///
    /// Note: caller is responsible for making sure the requested `ImageFormat` is supported.
    // Clippy complains about a to_xxx method taking &mut self. Semantically speaking, this is not
    // really a mutation, so we'll keep the name. Consider using interior mutability in the future.
    #[allow(clippy::wrong_self_convention)]
    pub fn to_image_buf(&mut self, fmt: ImageFormat) -> Result<ImageBuf, piet::Error> {
        let width = self.ctx.width();
        let height = self.ctx.height();
        let mut buf = vec![0; width * height * 4];
        self.copy_raw_pixels(fmt, &mut buf)?;
        Ok(ImageBuf::from_raw(buf, fmt, width, height))
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
            return Err(Error::NotSupported);
        }

        let width = self.ctx.width();
        let height = self.ctx.height();
        let stride = self.ctx.bytes_per_row();
        let data = self.ctx.data();
        let size = width * height * 4;
        if buf.len() < size {
            return Err(piet::Error::InvalidInput);
        }
        let used_stride = width * 4;
        if stride != used_stride {
            for y in 0..height {
                let src_start = y * stride;
                let src_end = src_start + used_stride;
                let dst_start = y * used_stride;
                let dst_end = dst_start + used_stride;
                buf[dst_start..dst_end].copy_from_slice(&data[src_start..src_end])
            }
        } else {
            buf.copy_from_slice(data);
        }
        Ok(size)
    }

    /// Save bitmap to RGBA PNG file
    #[cfg(feature = "png")]
    pub fn save_to_file<P: AsRef<Path>>(mut self, path: P) -> Result<(), piet::Error> {
        let width = self.ctx.width();
        let height = self.ctx.height();
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
        Err(Error::MissingFeature("png"))
    }
}
