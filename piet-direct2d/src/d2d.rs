//! Convenience wrappers for Direct2D objects.
//!
//! These also function as safety boundaries (though determining the
//! exact safety guarantees is work in progress).

// TODO: get rid of this when we actually do use everything
#![allow(unused)]

use std::ffi::c_void;
use std::marker::PhantomData;
use std::ops::Deref;
use std::ptr::null_mut;

use wio::com::ComPtr;

use winapi::shared::dxgiformat::DXGI_FORMAT_R8G8B8A8_UNORM;
use winapi::shared::winerror::{HRESULT, SUCCEEDED};
use winapi::um::d2d1::{
    D2D1CreateFactory, ID2D1Brush, ID2D1Factory, ID2D1GeometrySink, ID2D1GradientStopCollection,
    ID2D1PathGeometry, ID2D1SolidColorBrush, D2D1_BEZIER_SEGMENT, D2D1_BRUSH_PROPERTIES,
    D2D1_COLOR_F, D2D1_DEBUG_LEVEL_WARNING, D2D1_EXTEND_MODE_CLAMP, D2D1_FACTORY_OPTIONS,
    D2D1_FACTORY_TYPE_MULTI_THREADED, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_BEGIN_HOLLOW,
    D2D1_FIGURE_END_CLOSED, D2D1_FIGURE_END_OPEN, D2D1_FILL_MODE_ALTERNATE, D2D1_FILL_MODE_WINDING,
    D2D1_GAMMA_2_2, D2D1_GRADIENT_STOP, D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES, D2D1_MATRIX_3X2_F,
    D2D1_POINT_2F, D2D1_QUADRATIC_BEZIER_SEGMENT, D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES,
    D2D1_SIZE_U,
};
use winapi::um::d2d1_1::{
    ID2D1Bitmap1, ID2D1DeviceContext, D2D1_BITMAP_OPTIONS_NONE, D2D1_BITMAP_PROPERTIES1,
};
use winapi::um::dcommon::{D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT};
use winapi::Interface;

use piet::FillRule;

#[derive(Debug)]
pub enum Error {
    WinapiError(HRESULT),
}

pub struct D2DFactory(ComPtr<ID2D1Factory>);

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

pub struct Brush(ComPtr<ID2D1Brush>);

pub struct Bitmap(ComPtr<ID2D1Bitmap1>);

impl From<HRESULT> for Error {
    fn from(hr: HRESULT) -> Error {
        Error::WinapiError(hr)
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
    pub fn new() -> Result<D2DFactory, Error> {
        unsafe {
            let mut ptr: *mut ID2D1Factory = null_mut();
            let hr = D2D1CreateFactory(
                D2D1_FACTORY_TYPE_MULTI_THREADED,
                &ID2D1Factory::uuidof(),
                &D2D1_FACTORY_OPTIONS {
                    debugLevel: D2D1_DEBUG_LEVEL_WARNING,
                },
                &mut ptr as *mut _ as *mut _,
            );
            wrap(hr, ptr, D2DFactory)
        }
    }

    pub fn create_path_geometry(&self) -> Result<PathGeometry, Error> {
        unsafe {
            let mut ptr = null_mut();
            let hr = self.0.CreatePathGeometry(&mut ptr);
            wrap(hr, ptr, PathGeometry)
        }
    }
}

const DEFAULT_BRUSH_PROPERTIES: D2D1_BRUSH_PROPERTIES = D2D1_BRUSH_PROPERTIES {
    opacity: 1.0,
    transform: D2D1_MATRIX_3X2_F {
        matrix: [[1.0, 0.0], [0.0, 1.0], [0.0, 0.0]],
    },
};

impl DeviceContext {
    pub fn new(ptr: ComPtr<ID2D1DeviceContext>) -> DeviceContext {
        DeviceContext(ptr)
    }

    pub fn pop_layer(&mut self) {
        unsafe {
            self.0.PopLayer();
        }
    }

    pub fn create_solid_color(&mut self, color: D2D1_COLOR_F) -> Result<Brush, Error> {
        unsafe {
            let mut ptr = null_mut();
            let hr = self
                .0
                .CreateSolidColorBrush(&color, &DEFAULT_BRUSH_PROPERTIES, &mut ptr);
            wrap(hr, ptr, |p| Brush(p.up()))
        }
    }

    pub fn create_gradient_stops(
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

    pub fn create_linear_gradient(
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

    pub fn create_radial_gradient(
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
    pub fn create_bitmap(
        &mut self,
        width: usize,
        height: usize,
        buf: &[u8],
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
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
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

    pub fn close(&mut self) -> Result<(), Error> {
        unsafe { wrap_unit(self.ptr.Close()) }
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
        let mut s2 = p.open().unwrap();
        s2.close();
    }
}
