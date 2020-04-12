//! Support for piet Web back-end.

#[doc(hidden)]
pub use piet_web::*;
pub type Piet<'a> = WebRenderContext<'a>;

/// The associated brush type for this backend.
///
/// This type matches `RenderContext::Brush`
pub type Brush = piet_web::Brush;

/// The associated text factory for this backend.
///
/// This type matches `RenderContext::Text`
pub type PietText<'a> = WebRenderContext<'a>;

/// The associated font type for this backend.
///
/// This type matches `RenderContext::Text::Font`
pub type PietFont = WebFont;

/// The associated font builder for this backend.
///
/// This type matches `RenderContext::Text::FontBuilder`
pub type PietFontBuilder<'a> = WebFontBuilder;

/// The associated text layout type for this backend.
///
/// This type matches `RenderContext::Text::TextLayout`
pub type PietTextLayout = WebTextLayout;

/// The associated text layout builder for this backend.
///
/// This type matches `RenderContext::Text::TextLayoutBuilder`
pub type PietTextLayoutBuilder<'a> = WebTextLayoutBuilder;

/// The associated image type for this backend.
///
/// This type matches `RenderContext::Image`
pub type Image = WebImage;

/// A struct that can be used to create bitmap render contexts.
pub struct Device;

/// A struct provides a `RenderContext` and then can have its bitmap extracted.
pub struct BitmapTarget<'a> {
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
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
        let document = web_sys::window().unwrap().document().unwrap();
        let canvas = document
            .create_element("canvas")
            .unwrap()
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .unwrap()
        let context = canvas
            .get_context("2d")
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
        WebRenderContext::new(self.ctx.clone(), web_sys::window().unwrap())
    }

    /// Get raw RGBA pixels from the bitmap.
    pub fn into_raw_pixels(mut self, fmt: ImageFormat) -> Result<Vec<u8>, piet::Error> {
        // TODO: This code is just a snippet. A thorough review and testing should be done before
        // this is used. It is here for compatibility with druid.
        if fmt != ImageFormat::RgbaPremul {
            return Err(piet::new_eror(ErrorKind::NotSupported));
        }

        let mut raw_data = vec![0; width * height * 4];
        let img_data = self.context.get_image_data()
            .map_err(Into::<Box<dyn std::error::Error>>::into)?;

        // ImageDate is in RGBA order. This should be the same as expected on the output.
        Ok(img_data.data().0)
    }
}
