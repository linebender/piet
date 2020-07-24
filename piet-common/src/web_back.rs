//! Support for piet Web back-end.

use std::fmt;
use std::marker::PhantomData;
use std::path::Path;

#[cfg(feature = "png")]
use std::fs::File;
#[cfg(feature = "png")]
use std::io::BufWriter;

#[cfg(feature = "png")]
use png::{ColorType, Encoder};
use wasm_bindgen::JsCast;

use piet::ImageFormat;
#[doc(hidden)]
pub use piet_web::*;

pub type Piet<'a> = WebRenderContext;

/// The associated brush type for this backend.
///
/// This type matches `RenderContext::Brush`
pub type Brush = piet_web::Brush;

/// The associated text factory for this backend.
///
/// This type matches `RenderContext::Text`
pub type PietText = WebText;

/// The associated text layout type for this backend.
///
/// This type matches `RenderContext::Text::TextLayout`
pub type PietTextLayout = WebTextLayout;

/// The associated text layout builder for this backend.
///
/// This type matches `RenderContext::Text::TextLayoutBuilder`
pub type PietTextLayoutBuilder = WebTextLayoutBuilder;

/// The associated image type for this backend.
///
/// This type matches `RenderContext::Image`
pub type Image = WebImage;

/// A struct that can be used to create bitmap render contexts.
pub struct Device {
    // Since not all backends can support `Device: Sync`, make it non-Sync here to, for fewer
    // portability surprises.
    marker: std::marker::PhantomData<*const ()>,
}

unsafe impl Send for Device {}

/// A struct provides a `RenderContext` and then can have its bitmap extracted.
pub struct BitmapTarget<'a> {
    canvas: web_sys::HtmlCanvasElement,
    context: web_sys::CanvasRenderingContext2d,
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
        let document = web_sys::window().unwrap().document().unwrap();
        let canvas = document
            .create_element("canvas")
            .unwrap()
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .unwrap();
        let context = canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .unwrap();

        canvas.set_width(width as u32);
        canvas.set_height(height as u32);
        let _ = context.scale(pix_scale, pix_scale);

        Ok(BitmapTarget {
            canvas,
            context,
            phantom: Default::default(),
        })
    }
}

impl<'a> BitmapTarget<'a> {
    /// Get a piet `RenderContext` for the bitmap.
    pub fn render_context(&mut self) -> WebRenderContext {
        WebRenderContext::new(self.context.clone(), web_sys::window().unwrap())
    }

    /// Get raw RGBA pixels from the bitmap.
    fn raw_pixels(&mut self, fmt: ImageFormat) -> Result<Vec<u8>, piet::Error> {
        // TODO: This code is just a snippet. A thorough review and testing should be done before
        // this is used. It is here for compatibility with druid.

        if fmt != ImageFormat::RgbaPremul {
            return Err(piet::Error::NotSupported);
        }

        let width = self.canvas.width() as usize;
        let height = self.canvas.height() as usize;

        let img_data = self
            .context
            .get_image_data(0.0, 0.0, width as f64, height as f64)
            .map_err(|jsv| piet::Error::BackendError(Box::new(JsError::new(jsv))))?;

        // ImageDate is in RGBA order. This should be the same as expected on the output.
        Ok(img_data.data().0)
    }

    /// Get raw RGBA pixels from the bitmap.
    #[deprecated(since = "0.2.0", note = "use raw_pixels")]
    pub fn into_raw_pixels(mut self, fmt: ImageFormat) -> Result<Vec<u8>, piet::Error> {
        self.raw_pixels(fmt)
    }

    /// Get raw RGBA pixels from the bitmap by copying them into `buf`. If all the pixels were
    /// copied, returns the number of bytes written. If `buf` wasn't big enough, returns an error
    /// and doesn't write anything.
    pub fn copy_raw_pixels(
        &mut self,
        fmt: ImageFormat,
        buf: &mut [u8],
    ) -> Result<usize, piet::Error> {
        let data = self.raw_pixels(fmt)?;
        if data.len() > buf.len() {
            return Err(piet::Error::InvalidInput);
        }
        buf.copy_from_slice(&data[..]);
        Ok(data.len())
    }

    /// Save bitmap to RGBA PNG file
    #[cfg(feature = "png")]
    pub fn save_to_file<P: AsRef<Path>>(mut self, path: P) -> Result<(), piet::Error> {
        let height = self.canvas.height();
        let width = self.canvas.width();
        let image = self.raw_pixels(ImageFormat::RgbaPremul)?;
        let file = BufWriter::new(File::create(path).map_err(Into::<Box<_>>::into)?);
        let mut encoder = Encoder::new(file, width as u32, height as u32);
        encoder.set_color(ColorType::RGBA);
        encoder
            .write_header()
            .map_err(Into::<Box<_>>::into)?
            .write_image_data(&image)
            .map_err(Into::<Box<_>>::into)?;
        Ok(())
    }

    /// Stub for feature is missing
    #[cfg(not(feature = "png"))]
    pub fn save_to_file<P: AsRef<Path>>(self, _path: P) -> Result<(), piet::Error> {
        Err(piet::Error::MissingFeature)
    }
}

#[derive(Clone, Debug)]
struct JsError {
    jsv: wasm_bindgen::JsValue,
}

impl JsError {
    fn new(jsv: wasm_bindgen::JsValue) -> Self {
        JsError { jsv }
    }
}

impl fmt::Display for JsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.jsv)
    }
}

impl std::error::Error for JsError {}
