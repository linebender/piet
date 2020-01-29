use std::ptr::null_mut;

// TODO figure out whether to export this or to move `raw_pixels` into this module.
pub use winapi::shared::dxgi::DXGI_MAP_READ;

use winapi::shared::dxgi::{IDXGIDevice, IDXGISurface};
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

#[derive(Debug)]
pub struct Error(HRESULT);

pub struct D3D11Device(ComPtr<ID3D11Device>);
pub struct D3D11DeviceContext(ComPtr<ID3D11DeviceContext>);
pub struct D3D11Texture2D(ComPtr<ID3D11Texture2D>);
pub struct DxgiDevice(ComPtr<IDXGIDevice>);

pub enum TextureMode {
    Target,
    Read,
}

impl TextureMode {
    pub fn usage(&self) -> D3D11_USAGE {
        match self {
            TextureMode::Target => D3D11_USAGE_DEFAULT,
            TextureMode::Read => D3D11_USAGE_STAGING,
        }
    }

    pub fn bind_flags(&self) -> D3D11_BIND_FLAG {
        match self {
            TextureMode::Target => D3D11_BIND_SHADER_RESOURCE | D3D11_BIND_RENDER_TARGET,
            TextureMode::Read => 0,
        }
    }

    pub fn cpu_access_flags(&self) -> D3D11_CPU_ACCESS_FLAG {
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
    pub fn inner(&self) -> &ComPtr<ID3D11Device> {
        &self.0
    }

    // This function only supports a fraction of available options.
    pub fn create() -> Result<(D3D11Device, D3D11DeviceContext), Error> {
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

    pub fn as_dxgi(&self) -> Option<DxgiDevice> {
        self.0.cast().ok().map(DxgiDevice)
    }

    pub fn create_texture(
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

impl D3D11DeviceContext {
    pub fn inner(&self) -> &ComPtr<ID3D11DeviceContext> {
        &self.0
    }
}

impl D3D11Texture2D {
    pub fn as_dxgi(&self) -> ComPtr<IDXGISurface> {
        self.0.cast().unwrap()
    }

    pub fn as_raw(&self) -> *mut ID3D11Texture2D {
        self.0.as_raw()
    }
}

impl DxgiDevice {
    pub fn as_raw(&self) -> *mut IDXGIDevice {
        self.0.as_raw()
    }
}
