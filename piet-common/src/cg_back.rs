// allows e.g. raw_data[dst_off + x * 4 + 2] = buf[src_off + x * 4 + 0];
#![allow(clippy::identity_op)]

//! Support for piet CoreGraphics back-end.

use std::marker::PhantomData;
use std::path::Path;
#[cfg(feature = "png")]
use std::{fs::File, io::BufWriter};

use core_graphics::{color_space::CGColorSpace, context::CGContext, image::CGImage};
#[cfg(feature = "png")]
use png::{ColorType, Encoder};

use piet::{Error, ImageFormat};
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
pub type PietText<'a> = CoreGraphicsText<'a>;

/// The associated font type for this backend.
///
/// This type matches `RenderContext::Text::Font`
pub type PietFont = CoreGraphicsFont;

/// The associated font builder for this backend.
///
/// This type matches `RenderContext::Text::FontBuilder`
pub type PietFontBuilder<'a> = CoreGraphicsFontBuilder;

/// The associated text layout type for this backend.
///
/// This type matches `RenderContext::Text::TextLayout`
pub type PietTextLayout = CoreGraphicsTextLayout;

/// The associated text layout builder for this backend.
///
/// This type matches `RenderContext::Text::TextLayoutBuilder`
pub type PietTextLayoutBuilder<'a> = CoreGraphicsTextLayout;

/// The associated image type for this backend.
///
/// This type matches `RenderContext::Image`
pub type Image = CGImage;

/// A struct that can be used to create bitmap render contexts.
pub struct Device;

/// A struct provides a `RenderContext` and then can have its bitmap extracted.
pub struct BitmapTarget<'a> {
    ctx: CGContext,
    height: f64,
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
        CoreGraphicsContext::new_y_up(&mut self.ctx, self.height)
    }

    /// Get raw RGBA pixels from the bitmap.
    pub fn into_raw_pixels(mut self, fmt: ImageFormat) -> Result<Vec<u8>, piet::Error> {
        // TODO: convert other formats.
        if fmt != ImageFormat::RgbaPremul {
            return Err(Error::NotSupported);
        }

        let data = self.ctx.data();
        Ok(data.to_owned())
    }

    /// Save bitmap to RGBA PNG file
    #[cfg(feature = "png")]
    pub fn save_to_file<P: AsRef<Path>>(self, path: P) -> Result<(), piet::Error> {
        let width = self.ctx.width() as usize;
        let height = self.ctx.height() as usize;
        let mut data = self.into_raw_pixels(ImageFormat::RgbaPremul)?;
        piet_coregraphics::unpremultiply(&mut data);
        let file = BufWriter::new(File::create(path).map_err(|e| Into::<Box<_>>::into(e))?);
        let mut encoder = Encoder::new(file, width as u32, height as u32);
        encoder.set_color(ColorType::RGBA);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .map_err(|e| Into::<Box<_>>::into(e))?
            .write_image_data(&data)
            .map_err(|e| Into::<Box<_>>::into(e))?;
        Ok(())
    }

    /// Stub for feature is missing
    #[cfg(not(feature = "png"))]
    pub fn save_to_file<P: AsRef<Path>>(self, _path: P) -> Result<(), piet::Error> {
        Err(Error::MissingFeature)
    }
}
