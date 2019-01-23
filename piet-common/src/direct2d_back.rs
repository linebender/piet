//! Support for piet Direct2D back-end.

pub use piet_direct2d::*;

use direct2d::enums::BitmapOptions;
use direct2d::image::Bitmap;
use direct2d::RenderTarget;
use direct3d11::flags::{BindFlags, CreateDeviceFlags};
use direct3d11::helpers::ComWrapper;
use dxgi::flags::Format;

/// The `RenderContext` for the Direct2D backend, which is selected.
pub type Piet<'a> = D2DRenderContext<'a>;

/// A struct that can be used to create bitmap render contexts.
pub struct Device {
    d2d: direct2d::Factory,
    dwrite: directwrite::Factory,
    d3d: direct3d11::Device,
    d3d_ctx: direct3d11::DeviceContext,
    device: direct2d::Device,
}

/// A struct provides a `RenderContext` and then can have its bitmap extracted.
pub struct BitmapTarget<'a> {
    width: usize,
    height: usize,
    d2d: &'a direct2d::Factory,
    dwrite: &'a directwrite::Factory,
    d3d: &'a direct3d11::Device,
    d3d_ctx: &'a direct3d11::DeviceContext,
    tex: direct3d11::Texture2D,
    context: direct2d::DeviceContext,
}

impl Device {
    /// Create a new device.
    ///
    /// This creates new Direct2D and DirectWrite factories, a Direct3D
    /// device, and a Direct2D device.
    pub fn new() -> Result<Device, piet::Error> {
        let d2d = direct2d::Factory::new().unwrap();
        let dwrite = directwrite::Factory::new().unwrap();

        // Initialize a D3D Device
        let (_, d3d, d3d_ctx) = direct3d11::Device::create()
            .with_flags(CreateDeviceFlags::BGRA_SUPPORT)
            .build()
            .unwrap();

        // Create the D2D Device and Context
        let device = direct2d::Device::create(&d2d, &d3d.as_dxgi()).unwrap();

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
        &self,
        width: usize,
        height: usize,
        pix_scale: f64,
    ) -> Result<BitmapTarget, piet::Error> {
        let mut context = direct2d::DeviceContext::create(&self.device, false).unwrap();

        // Create a texture to render to
        let tex = direct3d11::Texture2D::create(&self.d3d)
            .with_size(width as u32, height as u32)
            .with_format(Format::R8G8B8A8Unorm)
            .with_bind_flags(BindFlags::RENDER_TARGET | BindFlags::SHADER_RESOURCE)
            .build()
            .unwrap();

        // Bind the backing texture to a D2D Bitmap
        let target = Bitmap::create(&context)
            .with_dxgi_surface(&tex.as_dxgi())
            .with_dpi(96.0 * pix_scale as f32, 96.0 * pix_scale as f32)
            .with_options(BitmapOptions::TARGET)
            .build()
            .unwrap();

        context.set_target(&target);
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
    pub fn render_context<'b>(&'b mut self) -> D2DRenderContext<'b> {
        D2DRenderContext::new(self.d2d, self.dwrite, &mut self.context)
    }

    /// Get raw RGBA pixels from the bitmap.
    ///
    /// These are in premultiplied-alpha format.
    pub fn into_raw_pixels(mut self) -> Result<Vec<u8>, piet::Error> {
        self.context.end_draw().unwrap();
        let temp_texture = direct3d11::texture2d::Texture2D::create(self.d3d)
            .with_size(self.width as u32, self.height as u32)
            .with_format(direct3d11::flags::Format::R8G8B8A8Unorm)
            .with_bind_flags(direct3d11::flags::BindFlags::NONE)
            .with_usage(direct3d11::flags::Usage::Staging)
            .with_cpu_access_flags(direct3d11::flags::CpuAccessFlags::READ)
            .build()
            .unwrap();

        // TODO: Have a safe way to accomplish this :D
        let mut raw_pixels: Vec<u8> = Vec::with_capacity(self.width * self.height * 4);
        unsafe {
            let ctx = &*self.d3d_ctx.get_raw();
            ctx.CopyResource(
                temp_texture.get_raw() as *mut _,
                self.tex.get_raw() as *mut _,
            );
            ctx.Flush();

            let surface = temp_texture.as_dxgi();
            let map = surface.map(true, false, false).unwrap();
            for y in 0..(self.height as u32) {
                raw_pixels.extend_from_slice(&map.row(y)[..self.width * 4]);
            }
        }
        Ok(raw_pixels)
    }
}
