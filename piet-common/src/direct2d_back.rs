//! Support for piet Direct2D back-end.

use std::fmt;

use direct2d::enums::BitmapOptions;
use direct2d::image::Bitmap;
use direct2d::render_target::RenderTag;
use direct2d::RenderTarget;
use direct3d11::flags::{BindFlags, CreateDeviceFlags};
use direct3d11::helpers::ComWrapper;
use dxgi::flags::Format;

use piet::{ErrorKind, ImageFormat};

pub use piet_direct2d::*;

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

trait WrapError<T> {
    fn wrap(self) -> Result<T, piet::Error>;
}

#[derive(Debug)]
struct WrappedD2DTag(direct2d::Error, Option<RenderTag>);

#[derive(Debug)]
struct WrappedD3D11Error(direct3d11::Error);

#[derive(Debug)]
struct WrappedDxgiError(dxgi::Error);

impl std::error::Error for WrappedD2DTag {}
impl std::error::Error for WrappedD3D11Error {}
impl std::error::Error for WrappedDxgiError {}

impl fmt::Display for WrappedD2DTag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Direct2D error: {}, tag {:?}", self.0, self.1)
    }
}

impl fmt::Display for WrappedD3D11Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Direct3D11 error: {}", self.0)
    }
}

impl fmt::Display for WrappedDxgiError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Dxgi error: {}", self.0)
    }
}

impl<T> WrapError<T> for Result<T, (direct2d::Error, Option<RenderTag>)> {
    fn wrap(self) -> Result<T, piet::Error> {
        self.map_err(|(e, t)| {
            let e: Box<dyn std::error::Error> = Box::new(WrappedD2DTag(e, t));
            e.into()
        })
    }
}

impl<T> WrapError<T> for Result<T, direct3d11::Error> {
    fn wrap(self) -> Result<T, piet::Error> {
        self.map_err(|e| {
            let e: Box<dyn std::error::Error> = Box::new(WrappedD3D11Error(e));
            e.into()
        })
    }
}

impl<T> WrapError<T> for Result<T, dxgi::Error> {
    fn wrap(self) -> Result<T, piet::Error> {
        self.map_err(|e| {
            let e: Box<dyn std::error::Error> = Box::new(WrappedDxgiError(e));
            e.into()
        })
    }
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
    pub fn into_raw_pixels(mut self, fmt: ImageFormat) -> Result<Vec<u8>, piet::Error> {
        // TODO: convert other formats.
        if fmt != ImageFormat::RgbaPremul {
            return Err(piet::new_error(ErrorKind::NotSupported));
        }
        self.context.end_draw().wrap()?;
        let temp_texture = direct3d11::texture2d::Texture2D::create(self.d3d)
            .with_size(self.width as u32, self.height as u32)
            .with_format(direct3d11::flags::Format::R8G8B8A8Unorm)
            .with_bind_flags(direct3d11::flags::BindFlags::NONE)
            .with_usage(direct3d11::flags::Usage::Staging)
            .with_cpu_access_flags(direct3d11::flags::CpuAccessFlags::READ)
            .build()
            .wrap()?;

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
            let map = surface.map(true, false, false).wrap()?;
            for y in 0..(self.height as u32) {
                raw_pixels.extend_from_slice(&map.row(y)[..self.width * 4]);
            }
        }
        Ok(raw_pixels)
    }
}
