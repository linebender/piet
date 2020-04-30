//! Support for piet Direct2D back-end.

#[cfg(feature = "png")]
use std::fs::File;
#[cfg(feature = "png")]
use std::io::BufWriter;
use std::path::Path;

/// For saving to file functionality
#[cfg(feature = "png")]
use png::{ColorType, Encoder};

use piet::ImageFormat;
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
pub type PietText<'a> = D2DText<'a>;

/// The associated font type for this backend.
///
/// This type matches `RenderContext::Text::Font`
pub type PietFont = D2DFont;

/// The associated font builder for this backend.
///
/// This type matches `RenderContext::Text::FontBuilder`
pub type PietFontBuilder<'a> = D2DFontBuilder<'a>;

/// The associated text layout type for this backend.
///
/// This type matches `RenderContext::Text::TextLayout`
pub type PietTextLayout = D2DTextLayout;

/// The associated text layout builder for this backend.
///
/// This type matches `RenderContext::Text::TextLayoutBuilder`
pub type PietTextLayoutBuilder<'a> = D2DTextLayoutBuilder<'a>;

/// The associated image type for this backend.
///
/// This type matches `RenderContext::Image`
pub type Image = Bitmap;

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

trait WrapError<T> {
    fn wrap(self) -> Result<T, piet::Error>;
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
    ) -> Result<BitmapTarget, piet::Error> {
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
    pub fn render_context(&mut self) -> D2DRenderContext {
        D2DRenderContext::new(self.d2d, self.dwrite, &mut self.context)
    }

    /// Get raw RGBA pixels from the bitmap.
    pub fn into_raw_pixels(mut self, fmt: ImageFormat) -> Result<Vec<u8>, piet::Error> {
        self.context.end_draw()?;
        // TODO: convert other formats.
        if fmt != ImageFormat::RgbaPremul {
            return Err(piet::Error::NotSupported);
        }
        let temp_texture = self
            .d3d
            .create_texture(self.width as u32, self.height as u32, TextureMode::Read)
            .unwrap();

        // TODO: Have a safe way to accomplish this :D
        let mut raw_pixels: Vec<u8> = Vec::with_capacity(self.width * self.height * 4);
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
                let dst = raw_pixels
                    .as_mut_ptr()
                    .offset(self.width as isize * 4 * y as isize);
                std::ptr::copy_nonoverlapping(src, dst, self.width * 4);
            }
            raw_pixels.set_len(self.width * self.height * 4);
        }
        Ok(raw_pixels)
    }

    /// Save bitmap to RGBA PNG file
    #[cfg(feature = "png")]
    pub fn save_to_file<P: AsRef<Path>>(self, path: P) -> Result<(), piet::Error> {
        let height = self.height;
        let width = self.width;
        let image = self.into_raw_pixels(ImageFormat::RgbaPremul)?;
        let file = BufWriter::new(File::create(path).map_err(|e| Into::<Box<_>>::into(e))?);
        let mut encoder = Encoder::new(file, width as u32, height as u32);
        encoder.set_color(ColorType::RGBA);
        encoder
            .write_header()
            .map_err(|e| Into::<Box<_>>::into(e))?
            .write_image_data(&image)
            .map_err(|e| Into::<Box<_>>::into(e))?;
        Ok(())
    }

    /// Stub for feature is missing
    #[cfg(not(feature = "png"))]
    pub fn save_to_file<P: AsRef<Path>>(self, _path: P) -> Result<(), piet::Error> {
        Err(piet::Error::MissingFeature)
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
    fn into_raw_pixels() {
        let mut device = Device::new().unwrap();
        let mut target = device.bitmap_target(640, 480, 1.0).unwrap();
        let mut piet = target.render_context();
        piet.clip(Rect::ZERO);
        piet.finish().unwrap();
        std::mem::drop(piet);
        target.into_raw_pixels(ImageFormat::RgbaPremul).unwrap();
    }
}
