// Copyright 2019 the Piet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Support for piet Direct2D back-end.

#[cfg(feature = "png")]
use std::fs::File;
#[cfg(feature = "png")]
use std::io::BufWriter;
use std::path::Path;

/// For saving to file functionality
#[cfg(feature = "png")]
use png::{ColorType, Encoder};

#[cfg(feature = "png")]
use piet::util;
use piet::{ImageBuf, ImageFormat};
use piet_direct2d::d2d::{Bitmap, Brush as D2DBrush};
use piet_direct2d::d3d::{
    D3D11Device, D3D11DeviceContext, D3D11Texture2D, TextureMode, DXGI_MAP_READ,
};
#[doc(hidden)]
pub use piet_direct2d::*;

/// The `RenderContext` for the Direct2D backend, which is selected.
pub type Piet<'a> = D2DRenderContext<'a>;

/// The associated brush type for this backend.
///
/// This type matches `RenderContext::Brush`
pub type Brush = D2DBrush;

/// The associated text factory for this backend.
///
/// This type matches `RenderContext::Text`
pub type PietText = D2DText;

/// The associated text layout type for this backend.
///
/// This type matches `RenderContext::Text::TextLayout`
pub type PietTextLayout = D2DTextLayout;

/// The associated text layout builder for this backend.
///
/// This type matches `RenderContext::Text::TextLayoutBuilder`
pub type PietTextLayoutBuilder = D2DTextLayoutBuilder;

/// The associated image type for this backend.
///
/// This type matches `RenderContext::Image`
pub type PietImage = Bitmap;

/// A struct that can be used to create bitmap render contexts.
pub struct Device {
    d2d: D2DFactory,
    dwrite: DwriteFactory,
    d3d: D3D11Device,
    d3d_ctx: D3D11DeviceContext,
    device: D2DDevice,
}

/// A struct provides a `RenderContext` and then can have its bitmap extracted.
pub struct BitmapTarget<'a> {
    width: usize,
    height: usize,
    d2d: &'a D2DFactory,
    dwrite: &'a DwriteFactory,
    d3d: &'a D3D11Device,
    d3d_ctx: &'a D3D11DeviceContext,
    tex: D3D11Texture2D,
    context: D2DDeviceContext,
}

impl Device {
    /// Create a new device.
    ///
    /// This creates new Direct2D and DirectWrite factories, a Direct3D
    /// device, and a Direct2D device.
    pub fn new() -> Result<Device, piet::Error> {
        let d2d = D2DFactory::new().unwrap();
        let dwrite = DwriteFactory::new().unwrap();

        // Initialize a D3D Device
        let (d3d, d3d_ctx) = D3D11Device::create().unwrap();

        // Create the D2D Device
        let device = unsafe { d2d.create_device(d3d.as_dxgi().unwrap().as_raw()).unwrap() };

        Ok(Device {
            d2d,
            dwrite,
            d3d,
            d3d_ctx,
            device,
        })
    }

    /// Create a new bitmap target.
    pub fn bitmap_target(
        &mut self,
        width: usize,
        height: usize,
        pix_scale: f64,
    ) -> Result<BitmapTarget<'_>, piet::Error> {
        let mut context = self.device.create_device_context().unwrap();

        // Create a texture to render to
        let tex = self
            .d3d
            .create_texture(width as u32, height as u32, TextureMode::Target)
            .unwrap();

        // Bind the backing texture to a D2D Bitmap
        let target = unsafe {
            context
                .create_bitmap_from_dxgi(&tex.as_dxgi(), pix_scale as f32)
                .unwrap()
        };

        context.set_target(&target);
        // TODO ask about this? it was in basic.rs, but not here
        context.set_dpi_scale(pix_scale as f32);
        context.begin_draw();

        Ok(BitmapTarget {
            width,
            height,
            d2d: &self.d2d,
            dwrite: &self.dwrite,
            d3d: &self.d3d,
            d3d_ctx: &self.d3d_ctx,
            tex,
            context,
        })
    }
}

impl<'a> BitmapTarget<'a> {
    /// Get a piet `RenderContext` for the bitmap.
    ///
    /// Note: caller is responsible for calling `finish` on the render
    /// context at the end of rendering.
    pub fn render_context(&mut self) -> D2DRenderContext<'_> {
        let text = D2DText::new_with_shared_fonts(self.dwrite.clone(), None);
        D2DRenderContext::new(self.d2d, text, &mut self.context)
    }

    /// Get an in-memory pixel buffer from the bitmap.
    ///
    /// Note: caller is responsible for making sure the requested `ImageFormat` is supported.
    // Clippy complains about a to_xxx method taking &mut self. Semantically speaking, this is not
    // really a mutation, so we'll keep the name. Consider using interior mutability in the future.
    #[allow(clippy::wrong_self_convention)]
    pub fn to_image_buf(&mut self, fmt: ImageFormat) -> Result<ImageBuf, piet::Error> {
        let mut buf = vec![0; self.width * self.height * 4];
        self.copy_raw_pixels(fmt, &mut buf)?;
        Ok(ImageBuf::from_raw(buf, fmt, self.width, self.height))
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
        let draw_restarter = self.context.end_draw_temporarily()?;
        let temp_texture = self
            .d3d
            .create_texture(self.width as u32, self.height as u32, TextureMode::Read)
            .unwrap();

        let size = self.width * self.height * 4;
        if size > buf.len() {
            return Err(piet::Error::InvalidInput);
        }

        // TODO: Have a safe way to accomplish this :D
        unsafe {
            self.d3d_ctx
                .inner()
                .CopyResource(temp_texture.as_raw() as *mut _, self.tex.as_raw() as *mut _);
            self.d3d_ctx.inner().Flush();

            let surface = temp_texture.as_dxgi();
            let mut mapped_rect = std::mem::zeroed();
            let _hr = surface.Map(&mut mapped_rect, DXGI_MAP_READ);
            for y in 0..self.height {
                let src = mapped_rect
                    .pBits
                    .offset(mapped_rect.Pitch as isize * y as isize);
                let dst = buf
                    .as_mut_ptr()
                    .offset(self.width as isize * 4 * y as isize);
                std::ptr::copy_nonoverlapping(src, dst, self.width * 4);
            }
        }

        drop(draw_restarter); // Make sure the restarter lives until this point for the happy path
        Ok(size)
    }

    /// Save bitmap to RGBA PNG file
    #[cfg(feature = "png")]
    pub fn save_to_file<P: AsRef<Path>>(mut self, path: P) -> Result<(), piet::Error> {
        let width = self.width;
        let height = self.height;
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
        Err(piet::Error::MissingFeature("png"))
    }
}

impl<'a> Drop for BitmapTarget<'a> {
    fn drop(&mut self) {
        let _ = self.context.end_draw();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use piet::kurbo::*;
    use piet::*;

    #[test]
    fn bitmap_target_drop() {
        let mut device = Device::new().unwrap();
        let bitmap_target = device.bitmap_target(640, 480, 1.0).unwrap();
        std::mem::drop(bitmap_target);
    }

    #[test]
    fn to_image_buf() {
        let mut device = Device::new().unwrap();
        let mut target = device.bitmap_target(640, 480, 1.0).unwrap();
        let mut piet = target.render_context();
        piet.clip(Rect::ZERO);
        piet.finish().unwrap();
        std::mem::drop(piet);
        target.to_image_buf(ImageFormat::RgbaPremul).unwrap();
    }

    #[test]
    fn copy_raw_pixels_and_continue() {
        let mut device = Device::new().unwrap();
        let mut bitmap = device.bitmap_target(64, 64, 1.0).unwrap();
        {
            let mut rc = bitmap.render_context();
            let rect = Rect::new(4.0, 4.0, 8.0, 8.0);
            rc.fill(rect, &Color::RED);
            rc.finish().unwrap();
        }
        let mut data = vec![0; 64 * 64 * 4];
        if !matches!(
            bitmap.copy_raw_pixels(ImageFormat::RgbaSeparate, &mut data),
            Err(Error::NotSupported)
        ) {
            panic!("Expected image format to not be supported, but something else happened.")
        }
        let mut invalid_data = vec![0; 1];
        if !matches!(
            bitmap.copy_raw_pixels(ImageFormat::RgbaPremul, &mut invalid_data),
            Err(Error::InvalidInput)
        ) {
            panic!("Expected invalid input error, but something else happened.")
        }
        bitmap
            .copy_raw_pixels(ImageFormat::RgbaPremul, &mut data)
            .unwrap();
        {
            let mut rc = bitmap.render_context();
            let rect = Rect::new(6.0, 6.0, 10.0, 10.0);
            rc.fill(rect, &Color::GREEN);
            rc.finish().unwrap();
        }
        bitmap
            .copy_raw_pixels(ImageFormat::RgbaPremul, &mut data)
            .unwrap();
    }
}
