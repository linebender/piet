//! Convenience wrappers for Direct2D objects.
//!
//! These also function as safety boundaries (though determining the
//! exact safety guarantees is work in progress).

// TODO: get rid of this when we actually do use everything
#![allow(unused)]

use std::ffi::c_void;
use std::fmt::{Debug, Display, Formatter};
use std::marker::PhantomData;
use std::ops::Deref;
use std::ptr::{null, null_mut};

use wio::com::ComPtr;

use winapi::shared::dxgi::{IDXGIDevice, IDXGISurface};
use winapi::shared::dxgiformat::DXGI_FORMAT_R8G8B8A8_UNORM;
use winapi::shared::winerror::{HRESULT, SUCCEEDED};
use winapi::um::d2d1::{
    D2D1CreateFactory, ID2D1Bitmap, ID2D1Brush, ID2D1Geometry, ID2D1GeometrySink,
    ID2D1GradientStopCollection, ID2D1Image, ID2D1Layer, ID2D1PathGeometry, ID2D1SolidColorBrush,
    ID2D1StrokeStyle, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_BEZIER_SEGMENT,
    D2D1_BITMAP_INTERPOLATION_MODE, D2D1_BRUSH_PROPERTIES, D2D1_COLOR_F, D2D1_DEBUG_LEVEL_WARNING,
    D2D1_DRAW_TEXT_OPTIONS, D2D1_EXTEND_MODE_CLAMP, D2D1_FACTORY_OPTIONS,
    D2D1_FACTORY_TYPE_MULTI_THREADED, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_BEGIN_HOLLOW,
    D2D1_FIGURE_END_CLOSED, D2D1_FIGURE_END_OPEN, D2D1_FILL_MODE_ALTERNATE, D2D1_FILL_MODE_WINDING,
    D2D1_GAMMA_2_2, D2D1_GRADIENT_STOP, D2D1_LAYER_OPTIONS_NONE, D2D1_LAYER_PARAMETERS,
    D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES, D2D1_MATRIX_3X2_F, D2D1_POINT_2F,
    D2D1_QUADRATIC_BEZIER_SEGMENT, D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES, D2D1_RECT_F, D2D1_SIZE_F,
    D2D1_SIZE_U, D2D1_STROKE_STYLE_PROPERTIES,
};
use winapi::um::d2d1_1::{
    ID2D1Bitmap1, ID2D1Device, ID2D1DeviceContext, ID2D1Factory1, D2D1_BITMAP_OPTIONS_NONE,
    D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1, D2D1_DEVICE_CONTEXT_OPTIONS_NONE,
};
use winapi::um::dcommon::{D2D1_ALPHA_MODE, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT};
use winapi::Interface;

use piet::{new_error, ErrorKind};

use crate::dwrite::TextLayout;

pub enum FillRule {
    EvenOdd,
    NonZero,
}

pub enum Error {
    WinapiError(HRESULT),
}

/// A Direct2D factory object.
///
/// This struct is public only to use for system integration in piet_common and druid-shell. It is not intended
/// that end-users directly use this struct.
pub struct D2DFactory(ComPtr<ID2D1Factory1>);

/// A Direct2D device.
pub struct D2DDevice(ComPtr<ID2D1Device>);

/// The main context that takes drawing operations.
///
/// This type is a thin wrapper for
/// [ID2D1DeviceContext](https://docs.microsoft.com/en-us/windows/win32/api/d2d1_1/nn-d2d1_1-id2d1devicecontext).
///
/// This struct is public only to use for system integration in piet_common and druid-shell. It is not intended
/// that end-users directly use this struct.
pub struct DeviceContext(ComPtr<ID2D1DeviceContext>);

pub struct PathGeometry(ComPtr<ID2D1PathGeometry>);

pub struct GeometrySink<'a> {
    ptr: ComPtr<ID2D1GeometrySink>,
    // The PhantomData keeps us from doing stuff like having
    // two GeometrySink objects open on the same PathGeometry.
    //
    // It's conservative, but helps avoid logic errors.
    marker: PhantomData<&'a mut PathGeometry>,
}

pub struct GradientStopCollection(ComPtr<ID2D1GradientStopCollection>);

// TODO: consider not building this at all, but just Brush.
pub struct SolidColorBrush(ComPtr<ID2D1SolidColorBrush>);

pub struct StrokeStyle(ComPtr<ID2D1StrokeStyle>);

pub struct Layer(ComPtr<ID2D1Layer>);

#[derive(Clone)]
pub struct Brush(ComPtr<ID2D1Brush>);

pub struct Bitmap(ComPtr<ID2D1Bitmap1>);

impl From<HRESULT> for Error {
    fn from(hr: HRESULT) -> Error {
        Error::WinapiError(hr)
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            Error::WinapiError(hr) => write!(f, "hresult {:x}", hr),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            Error::WinapiError(hr) => write!(f, "hresult {:x}", hr),
        }
    }
}

impl std::error::Error for Error {
    fn description(&self) -> &str {
        "winapi error"
    }
}

impl From<Error> for piet::Error {
    fn from(e: Error) -> piet::Error {
        new_error(ErrorKind::BackendError(Box::new(e)))
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
        Err(hr.into())
    }
}

fn wrap_unit(hr: HRESULT) -> Result<(), Error> {
    if SUCCEEDED(hr) {
        Ok(())
    } else {
        Err(hr.into())
    }
}

impl D2DFactory {
    /// Create a new Direct2D factory.
    ///
    /// This requires Windows 7 platform update, and can also fail if
    /// resources are unavailable.
    pub fn new() -> Result<D2DFactory, Error> {
        unsafe {
            let mut ptr: *mut ID2D1Factory1 = null_mut();
            let hr = D2D1CreateFactory(
                D2D1_FACTORY_TYPE_MULTI_THREADED,
                &ID2D1Factory1::uuidof(),
                &D2D1_FACTORY_OPTIONS {
                    debugLevel: D2D1_DEBUG_LEVEL_WARNING,
                },
                &mut ptr as *mut _ as *mut _,
            );
            wrap(hr, ptr, D2DFactory)
        }
    }

    // Would it be safe to take &ComPtr<IDXGIDevice> here?
    pub unsafe fn create_device(&self, dxgi_device: *mut IDXGIDevice) -> Result<D2DDevice, Error> {
        let mut ptr = null_mut();
        let hr = self.0.CreateDevice(dxgi_device, &mut ptr);
        wrap(hr, ptr, D2DDevice)
    }

    /// Get the raw pointer
    pub fn get_raw(&self) -> *mut ID2D1Factory1 {
        self.0.as_raw()
    }

    pub fn create_path_geometry(&self) -> Result<PathGeometry, Error> {
        unsafe {
            let mut ptr = null_mut();
            let hr = self.0.deref().deref().CreatePathGeometry(&mut ptr);
            wrap(hr, ptr, PathGeometry)
        }
    }

    pub fn create_stroke_style(
        &self,
        props: &D2D1_STROKE_STYLE_PROPERTIES,
        dashes: Option<&[f32]>,
    ) -> Result<StrokeStyle, Error> {
        unsafe {
            let mut ptr = null_mut();
            let dashes_len = dashes.map(|d| d.len()).unwrap_or(0);
            assert!(dashes_len <= 0xffff_ffff);
            let hr = self.0.deref().deref().CreateStrokeStyle(
                props,
                dashes.map(|d| d.as_ptr()).unwrap_or(null()),
                dashes_len as u32,
                &mut ptr,
            );
            wrap(hr, ptr, StrokeStyle)
        }
    }
}

impl D2DDevice {
    /// Create a new device context from the device.
    ///
    /// This is a wrapper for
    /// [ID2D1Device::CreateDeviceContext](https://docs.microsoft.com/en-us/windows/win32/api/d2d1_1/nf-d2d1_1-id2d1device-createdevicecontext).
    pub fn create_device_context(&mut self) -> Result<DeviceContext, Error> {
        unsafe {
            let mut ptr = null_mut();
            let options = D2D1_DEVICE_CONTEXT_OPTIONS_NONE;
            let hr = self.0.CreateDeviceContext(options, &mut ptr);
            wrap(hr, ptr, DeviceContext)
        }
    }
}

const IDENTITY_MATRIX_3X2_F: D2D1_MATRIX_3X2_F = D2D1_MATRIX_3X2_F {
    matrix: [[1.0, 0.0], [0.0, 1.0], [0.0, 0.0]],
};

const DEFAULT_BRUSH_PROPERTIES: D2D1_BRUSH_PROPERTIES = D2D1_BRUSH_PROPERTIES {
    opacity: 1.0,
    transform: IDENTITY_MATRIX_3X2_F,
};

impl DeviceContext {
    /// Create a new device context from an existing COM object.
    ///
    /// Marked as unsafe because the device must be in a good state.
    /// This *might* be overly conservative.
    pub unsafe fn new(ptr: ComPtr<ID2D1DeviceContext>) -> DeviceContext {
        DeviceContext(ptr)
    }

    /// Get the raw pointer
    pub fn get_raw(&self) -> *mut ID2D1DeviceContext {
        self.0.as_raw()
    }

    /// Get the Com ptr
    /// TODO rename to `inner`, like for D3D11Device?
    pub fn get_comptr(&self) -> &ComPtr<ID2D1DeviceContext> {
        &self.0
    }

    /// Create a bitmap from a DXGI surface.
    ///
    /// Most often, this bitmap will be used to set the target of a
    /// DeviceContext.
    ///
    /// Assumes RGBA8 format and premultiplied alpha.
    ///
    /// The `unsafe` might be conservative, but we assume the `dxgi`
    /// argument is in good shape to be a target.
    pub unsafe fn create_bitmap_from_dxgi(
        &self,
        dxgi: &ComPtr<IDXGISurface>,
        dpi_scale: f32,
    ) -> Result<Bitmap, Error> {
        let mut ptr = null_mut();
        let props = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_R8G8B8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0 * dpi_scale,
            dpiY: 96.0 * dpi_scale,
            bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
            colorContext: null_mut(),
        };
        let hr = self
            .0
            .CreateBitmapFromDxgiSurface(dxgi.as_raw(), &props, &mut ptr);
        wrap(hr, ptr, Bitmap)
    }

    /// Set the target for the device context.
    ///
    /// Useful for rendering into bitmaps.
    pub fn set_target(&mut self, target: &Bitmap) {
        unsafe { self.0.SetTarget(target.0.as_raw() as *mut ID2D1Image) }
    }

    /// Set the dpi scale.
    ///
    /// Mostly useful when rendering into bitmaps.
    pub fn set_dpi_scale(&mut self, dpi_scale: f32) {
        unsafe {
            self.0.SetDpi(96. * dpi_scale, 96. * dpi_scale);
        }
    }

    /// Begin drawing.
    ///
    /// This must be done before any piet drawing operations.
    ///
    /// There may be safety concerns (not clear what happens if the sequence
    /// is not followed).
    pub fn begin_draw(&mut self) {
        unsafe {
            self.0.BeginDraw();
        }
    }

    /// End drawing.
    pub fn end_draw(&mut self) -> Result<(), Error> {
        unsafe {
            let mut tag1 = 0;
            let mut tag2 = 0;
            let hr = self.0.EndDraw(&mut tag1, &mut tag2);
            wrap_unit(hr)
        }
    }

    pub(crate) fn clear(&mut self, color: D2D1_COLOR_F) {
        unsafe {
            self.0.Clear(&color);
        }
    }

    pub(crate) fn set_transform(&mut self, transform: &D2D1_MATRIX_3X2_F) {
        unsafe {
            self.0.SetTransform(transform);
        }
    }

    pub(crate) fn fill_geometry(
        &mut self,
        geom: &PathGeometry,
        brush: &Brush,
        opacity_brush: Option<&Brush>,
    ) {
        unsafe {
            self.0.FillGeometry(
                geom.0.as_raw() as *mut ID2D1Geometry,
                brush.0.as_raw(),
                opacity_brush.map(|b| b.0.as_raw()).unwrap_or(null_mut()),
            );
        }
    }

    pub(crate) fn draw_geometry(
        &mut self,
        geom: &PathGeometry,
        brush: &Brush,
        width: f32,
        style: Option<&StrokeStyle>,
    ) {
        unsafe {
            self.0.DrawGeometry(
                geom.0.as_raw() as *mut ID2D1Geometry,
                brush.0.as_raw(),
                width,
                style.map(|b| b.0.as_raw()).unwrap_or(null_mut()),
            );
        }
    }

    pub(crate) fn create_layer(&mut self, size: Option<D2D1_SIZE_F>) -> Result<Layer, Error> {
        unsafe {
            let mut ptr = null_mut();
            let hr = self.0.CreateLayer(
                size.as_ref().map(|r| r as *const _).unwrap_or(null()),
                &mut ptr,
            );
            wrap(hr, ptr, Layer)
        }
    }

    // Should be &mut layer?
    pub(crate) fn push_layer_mask(&mut self, mask: &PathGeometry, layer: &Layer) {
        unsafe {
            let params = D2D1_LAYER_PARAMETERS {
                contentBounds: D2D1_RECT_F {
                    left: std::f32::NEG_INFINITY,
                    top: std::f32::NEG_INFINITY,
                    right: std::f32::INFINITY,
                    bottom: std::f32::INFINITY,
                },
                geometricMask: mask.0.as_raw() as *mut ID2D1Geometry,
                maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                maskTransform: IDENTITY_MATRIX_3X2_F,
                opacity: 1.0,
                opacityBrush: null_mut(),
                layerOptions: D2D1_LAYER_OPTIONS_NONE,
            };
            self.0.deref().deref().PushLayer(&params, layer.0.as_raw());
        }
    }

    pub(crate) fn pop_layer(&mut self) {
        unsafe {
            self.0.PopLayer();
        }
    }

    pub(crate) fn create_solid_color(&mut self, color: D2D1_COLOR_F) -> Result<Brush, Error> {
        unsafe {
            let mut ptr = null_mut();
            let hr = self
                .0
                .CreateSolidColorBrush(&color, &DEFAULT_BRUSH_PROPERTIES, &mut ptr);
            wrap(hr, ptr, |p| Brush(p.up()))
        }
    }

    pub(crate) fn create_gradient_stops(
        &mut self,
        stops: &[D2D1_GRADIENT_STOP],
    ) -> Result<GradientStopCollection, Error> {
        unsafe {
            // Should this assert or should we return an overflow error? Super
            // unlikely in either case.
            assert!(stops.len() <= 0xffff_ffff);
            let mut ptr = null_mut();
            // The `deref` is because there is a method of the same name in DeviceContext
            // (with fancier color space controls). We'll take the vanilla one for now.
            let hr = self.0.deref().deref().CreateGradientStopCollection(
                stops.as_ptr(),
                stops.len() as u32,
                D2D1_GAMMA_2_2,
                D2D1_EXTEND_MODE_CLAMP,
                &mut ptr,
            );
            wrap(hr, ptr, GradientStopCollection)
        }
    }

    pub(crate) fn create_linear_gradient(
        &mut self,
        props: &D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES,
        stops: &GradientStopCollection,
    ) -> Result<Brush, Error> {
        unsafe {
            let mut ptr = null_mut();
            let hr = self.0.CreateLinearGradientBrush(
                props,
                &DEFAULT_BRUSH_PROPERTIES,
                stops.0.as_raw(),
                &mut ptr,
            );
            wrap(hr, ptr, |p| Brush(p.up()))
        }
    }

    pub(crate) fn create_radial_gradient(
        &mut self,
        props: &D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES,
        stops: &GradientStopCollection,
    ) -> Result<Brush, Error> {
        unsafe {
            let mut ptr = null_mut();
            let hr = self.0.CreateRadialGradientBrush(
                props,
                &DEFAULT_BRUSH_PROPERTIES,
                stops.0.as_raw(),
                &mut ptr,
            );
            wrap(hr, ptr, |p| Brush(p.up()))
        }
    }

    // Buf is always interpreted as RGBA32 premultiplied.
    pub(crate) fn create_bitmap(
        &mut self,
        width: usize,
        height: usize,
        buf: &[u8],
        alpha_mode: D2D1_ALPHA_MODE,
    ) -> Result<Bitmap, Error> {
        // Maybe using TryInto would be more Rust-like.
        // Note: value is set so that multiplying by 4 (for pitch) is valid.
        assert!(width <= 0x3fff_ffff);
        assert!(height <= 0xffff_ffff);
        let size = D2D1_SIZE_U {
            width: width as u32,
            height: height as u32,
        };
        let format = D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_R8G8B8A8_UNORM,
            alphaMode: alpha_mode,
        };
        let props = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: format,
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_NONE,
            colorContext: null_mut(),
        };
        let pitch = (width * 4) as u32;
        unsafe {
            let mut ptr = null_mut();
            let hr = self.0.deref().CreateBitmap(
                size,
                buf.as_ptr() as *const c_void,
                pitch,
                &props,
                &mut ptr,
            );
            wrap(hr, ptr, Bitmap)
        }
    }

    pub(crate) fn draw_text_layout(
        &mut self,
        origin: D2D1_POINT_2F,
        layout: &TextLayout,
        brush: &Brush,
        options: D2D1_DRAW_TEXT_OPTIONS,
    ) {
        unsafe {
            self.0
                .DrawTextLayout(origin, layout.get_raw(), brush.0.as_raw(), options);
        }
    }

    pub(crate) fn draw_bitmap(
        &mut self,
        bitmap: &Bitmap,
        dst_rect: &D2D1_RECT_F,
        opacity: f32,
        interp_mode: D2D1_BITMAP_INTERPOLATION_MODE,
        src_rect: Option<&D2D1_RECT_F>,
    ) {
        unsafe {
            // derefs are so we get RenderTarget method rather than DeviceContext method.
            // pointer casts are partly to undo that :)
            self.0.deref().deref().DrawBitmap(
                bitmap.0.as_raw() as *mut ID2D1Bitmap,
                dst_rect,
                opacity,
                interp_mode,
                src_rect.map(|r| r as *const _).unwrap_or(null()),
            );
        }
    }
}

impl PathGeometry {
    pub fn open<'a>(&'a mut self) -> Result<GeometrySink<'a>, Error> {
        unsafe {
            let mut ptr = null_mut();
            let hr = (self.0).Open(&mut ptr);
            wrap(hr, ptr, |ptr| GeometrySink {
                ptr,
                marker: Default::default(),
            })
        }
    }
}

// Note: this impl has not been audited for safety. It might be possible
// to provoke a crash by doing things in the wrong order.
impl<'a> GeometrySink<'a> {
    pub fn set_fill_mode(&mut self, fill_rule: FillRule) {
        let fill_mode = match fill_rule {
            FillRule::EvenOdd => D2D1_FILL_MODE_ALTERNATE,
            FillRule::NonZero => D2D1_FILL_MODE_WINDING,
        };
        unsafe {
            self.ptr.SetFillMode(fill_mode);
        }
    }

    pub fn add_bezier(
        &mut self,
        point1: D2D1_POINT_2F,
        point2: D2D1_POINT_2F,
        point3: D2D1_POINT_2F,
    ) {
        let seg = D2D1_BEZIER_SEGMENT {
            point1,
            point2,
            point3,
        };
        unsafe {
            self.ptr.AddBezier(&seg);
        }
    }

    pub fn add_quadratic_bezier(&mut self, point1: D2D1_POINT_2F, point2: D2D1_POINT_2F) {
        let seg = D2D1_QUADRATIC_BEZIER_SEGMENT { point1, point2 };
        unsafe {
            self.ptr.AddQuadraticBezier(&seg);
        }
    }

    pub fn add_line(&mut self, point: D2D1_POINT_2F) {
        unsafe {
            self.ptr.AddLine(point);
        }
    }

    pub fn begin_figure(&mut self, start: D2D1_POINT_2F, is_filled: bool) {
        unsafe {
            let figure_end = if is_filled {
                D2D1_FIGURE_BEGIN_FILLED
            } else {
                D2D1_FIGURE_BEGIN_HOLLOW
            };
            self.ptr.BeginFigure(start, figure_end);
        }
    }

    pub fn end_figure(&mut self, is_closed: bool) {
        unsafe {
            let figure_end = if is_closed {
                D2D1_FIGURE_END_CLOSED
            } else {
                D2D1_FIGURE_END_OPEN
            };
            self.ptr.EndFigure(figure_end);
        }
    }

    // A case can be made for doing this in the drop instead.
    pub fn close(self) -> Result<(), Error> {
        unsafe { wrap_unit(self.ptr.Close()) }
    }
}

/// This might not be needed.
impl Bitmap {
    pub fn get_size(&self) -> D2D1_SIZE_F {
        unsafe { self.0.GetSize() }
    }
}

mod tests {
    use super::*;

    #[test]
    fn geom_builder() {
        let mut factory = D2DFactory::new().unwrap();
        let mut p = factory.create_path_geometry().unwrap();
        let mut s1 = p.open().unwrap();
        // Note: if the next two lines are swapped, it's a compile
        // error.
        s1.close();
        if let Ok(mut s2) = p.open() {
            s2.close();
        }
    }
}
