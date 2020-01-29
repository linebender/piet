//! Basic example of rendering on Direct2D.
// TODO cleanup, much of this now exists in d3d.rs

use std::ptr::null_mut;

use winapi::shared::dxgi::{IDXGIDevice, IDXGISurface, DXGI_MAP_READ};
use winapi::shared::dxgiformat::DXGI_FORMAT_R8G8B8A8_UNORM;
use winapi::shared::dxgitype::DXGI_SAMPLE_DESC;
use winapi::shared::winerror::{HRESULT, SUCCEEDED};
use winapi::um::d3d11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, D3D11_BIND_FLAG,
    D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_CPU_ACCESS_FLAG,
    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE, D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING,
};
use winapi::um::d3dcommon::D3D_DRIVER_TYPE_HARDWARE;
use winapi::Interface;

use wio::com::ComPtr;

use piet::RenderContext;
use piet_direct2d::D2DRenderContext;

use piet_test::draw_test_picture;

const TEXTURE_WIDTH: u32 = 400;
const TEXTURE_HEIGHT: u32 = 200;

const TEXTURE_WIDTH_S: usize = TEXTURE_WIDTH as usize;
const TEXTURE_HEIGHT_S: usize = TEXTURE_HEIGHT as usize;

const HIDPI: f32 = 2.0;

fn main() {
    let test_picture_number = std::env::args()
        .skip(1)
        .next()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);

    // Create the D2D factory
    let d2d = piet_direct2d::D2DFactory::new().unwrap();
    let dwrite = piet_direct2d::DwriteFactory::new().unwrap();

    // Initialize a D3D Device
    let (d3d, d3d_ctx) = D3D11Device::create().unwrap();

    // Create the D2D Device and Context
    let mut device = unsafe { d2d.create_device(d3d.as_dxgi().unwrap().as_raw()).unwrap() };
    let mut context = device.create_device_context().unwrap();

    // Create a texture to render to
    let tex = d3d
        .create_texture(TEXTURE_WIDTH, TEXTURE_HEIGHT, TextureMode::Target)
        .unwrap();

    // Bind the backing texture to a D2D Bitmap
    let target = unsafe {
        context
            .create_bitmap_from_dxgi(&tex.as_dxgi(), HIDPI)
            .unwrap()
    };

    context.set_target(&target);
    context.set_dpi_scale(HIDPI);
    context.begin_draw();
    let mut piet_context = D2DRenderContext::new(&d2d, &dwrite, &mut context);
    // TODO: report errors more nicely than these unwraps.
    draw_test_picture(&mut piet_context, test_picture_number).unwrap();
    piet_context.finish().unwrap();
    context.end_draw().unwrap();

    let temp_texture = d3d
        .create_texture(TEXTURE_WIDTH, TEXTURE_HEIGHT, TextureMode::Read)
        .unwrap();

    // Get the data so we can write it to a file
    // TODO: Have a safe way to accomplish this :D
    let mut raw_pixels: Vec<u8> = Vec::with_capacity(TEXTURE_WIDTH_S * TEXTURE_HEIGHT_S * 4);
    unsafe {
        d3d_ctx
            .0
            .CopyResource(temp_texture.0.as_raw() as *mut _, tex.0.as_raw() as *mut _);
        d3d_ctx.0.Flush();

        let surface = temp_texture.as_dxgi();
        let mut mapped_rect = std::mem::zeroed();
        let _hr = surface.Map(&mut mapped_rect, DXGI_MAP_READ);
        for y in 0..TEXTURE_HEIGHT {
            let src = mapped_rect
                .pBits
                .offset(mapped_rect.Pitch as isize * y as isize);
            let dst = raw_pixels
                .as_mut_ptr()
                .offset(TEXTURE_WIDTH_S as isize * 4 * y as isize);
            std::ptr::copy_nonoverlapping(src, dst, TEXTURE_WIDTH_S * 4);
        }
        raw_pixels.set_len(TEXTURE_WIDTH_S * TEXTURE_HEIGHT_S * 4);
    }

    image::save_buffer(
        "temp-image.png",
        &raw_pixels,
        TEXTURE_WIDTH,
        TEXTURE_HEIGHT,
        image::ColorType::RGBA(8),
    )
    .unwrap();
}

// Minimal wrapping of enough bureaucracy to run the lib follows. It's
// possible we want to export some of this publicly for the sake of
// piet-common, avoiding code duplication.

#[derive(Debug)]
struct Error(HRESULT);

struct D3D11Device(ComPtr<ID3D11Device>);
struct D3D11DeviceContext(ComPtr<ID3D11DeviceContext>);
struct D3D11Texture2D(ComPtr<ID3D11Texture2D>);
struct DxgiDevice(ComPtr<IDXGIDevice>);

enum TextureMode {
    Target,
    Read,
}

impl TextureMode {
    fn usage(&self) -> D3D11_USAGE {
        match self {
            TextureMode::Target => D3D11_USAGE_DEFAULT,
            TextureMode::Read => D3D11_USAGE_STAGING,
        }
    }

    fn bind_flags(&self) -> D3D11_BIND_FLAG {
        match self {
            TextureMode::Target => D3D11_BIND_SHADER_RESOURCE | D3D11_BIND_RENDER_TARGET,
            TextureMode::Read => 0,
        }
    }

    fn cpu_access_flags(&self) -> D3D11_CPU_ACCESS_FLAG {
        match self {
            TextureMode::Target => 0,
            TextureMode::Read => D3D11_CPU_ACCESS_READ,
        }
    }
}

unsafe fn wrap<T, U, F>(hr: HRESULT, ptr: *mut T, f: F) -> Result<U, Error>
where
    F: Fn(ComPtr<T>) -> U,
    T: Interface,
{
    if SUCCEEDED(hr) {
        Ok(f(ComPtr::from_raw(ptr)))
    } else {
        Err(Error(hr))
    }
}

impl D3D11Device {
    // This function only supports a fraction of available options.
    fn create() -> Result<(D3D11Device, D3D11DeviceContext), Error> {
        unsafe {
            let mut ptr = null_mut();
            let mut ctx_ptr = null_mut();
            let hr = D3D11CreateDevice(
                null_mut(), /* adapter */
                D3D_DRIVER_TYPE_HARDWARE,
                null_mut(), /* module */
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                null_mut(), /* feature levels */
                0,
                D3D11_SDK_VERSION,
                &mut ptr,
                null_mut(), /* feature level */
                &mut ctx_ptr,
            );
            let device = wrap(hr, ptr, D3D11Device)?;
            let device_ctx = wrap(hr, ctx_ptr, D3D11DeviceContext)?;
            Ok((device, device_ctx))
        }
    }

    fn as_dxgi(&self) -> Option<DxgiDevice> {
        self.0.cast().ok().map(DxgiDevice)
    }

    fn create_texture(
        &self,
        width: u32,
        height: u32,
        mode: TextureMode,
    ) -> Result<D3D11Texture2D, Error> {
        unsafe {
            let mut ptr = null_mut();
            let desc = D3D11_TEXTURE2D_DESC {
                Width: width,
                Height: height,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: mode.usage(),
                BindFlags: mode.bind_flags(),
                CPUAccessFlags: mode.cpu_access_flags(),
                MiscFlags: 0,
            };
            let hr = self.0.CreateTexture2D(&desc, null_mut(), &mut ptr);
            wrap(hr, ptr, D3D11Texture2D)
        }
    }
}

impl DxgiDevice {
    fn as_raw(&self) -> *mut IDXGIDevice {
        self.0.as_raw()
    }
}

impl D3D11Texture2D {
    fn as_dxgi(&self) -> ComPtr<IDXGISurface> {
        self.0.cast().unwrap()
    }
}
