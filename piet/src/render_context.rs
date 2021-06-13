//! The main render context trait.

use std::borrow::Cow;

use kurbo::{Affine, Point, Rect, Shape};

use crate::{
    Color, Error, FixedGradient, FixedLinearGradient, FixedRadialGradient, Image, LinearGradient,
    RadialGradient, StrokeStyle, Text, TextLayout,
};

/// A requested interpolation mode for drawing images.
#[derive(Clone, Copy, PartialEq)]
pub enum InterpolationMode {
    /// Don't interpolate, use nearest neighbor.
    NearestNeighbor,
    /// Use bilinear interpolation.
    Bilinear,
}

/// The pixel format for bitmap images.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum ImageFormat {
    /// 1 byte per pixel.
    ///
    /// For example, a white pixel has value 0xff.
    Grayscale,
    /// 3 bytes per pixel, in RGB order.
    ///
    /// For example, a red pixel consists of three bytes `[0xff, 0, 0]` independent of the system's
    /// endianness.
    Rgb,
    /// 4 bytes per pixel, in RGBA order, with separate alpha.
    ///
    /// For example, a full-intensity red pixel with 50% transparency consists of four bytes
    /// `[0xff, 0, 0, 0x80]` independent of the system's endianness.
    RgbaSeparate,
    /// 4 bytes per pixel, in RGBA order, with premultiplied alpha.
    ///
    /// For example, a full-intensity red pixel with 50% transparency consists of four bytes
    /// `[0x80, 0, 0, 0x80]` independent of the system's endianness.
    RgbaPremul,
}

impl ImageFormat {
    /// The number of bytes required to represent a pixel in this format.
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            ImageFormat::Grayscale => 1,
            ImageFormat::Rgb => 3,
            ImageFormat::RgbaPremul | ImageFormat::RgbaSeparate => 4,
        }
    }
}

/// The main trait for rendering graphics.
///
/// This trait provides an API for drawing 2D graphics. In basic usage, it
/// wraps a surface of some kind, so that drawing commands paint onto the
/// surface. It can also be a recording context, creating a display list for
/// playback later.
///
/// The intent of the design is to be general so that any number of back-ends
/// can implement this trait.
///
/// Code that draws graphics will in general take `&mut impl RenderContext`.
pub trait RenderContext
where
    Self::Brush: IntoBrush<Self>,
{
    /// The type of a "brush".
    ///
    /// Represents solid colors and gradients.
    type Brush: Clone;

    /// An associated factory for creating text layouts and related resources.
    type Text: Text<TextLayout = Self::TextLayout>;

    /// The type use to represent text layout objects.
    type TextLayout: TextLayout;

    /// The associated type of an image.
    type Image: Image;

    /// Report an internal error.
    ///
    /// Drawing operations may cause internal errors, which may also occur
    /// asynchronously after the drawing command was issued. This method reports
    /// any such error that has been detected.
    fn status(&mut self) -> Result<(), Error>;

    /// Create a new brush resource.
    ///
    /// TODO: figure out how to document lifetime and rebuilding requirements. Should
    /// that be the responsibility of the client, or should the back-end take
    /// responsibility? We could have a cache that is flushed when the Direct2D
    /// render target is rebuilt. Solid brushes are super lightweight, but
    /// other potentially retained objects will be heavier.
    fn solid_brush(&mut self, color: Color) -> Self::Brush;

    /// Create a new gradient brush.
    fn gradient(&mut self, gradient: impl Into<FixedGradient>) -> Result<Self::Brush, Error>;

    /// Replace a region of the canvas with the provided [`Color`].
    ///
    /// The region can be omitted, in which case it will apply to the entire
    /// canvas.
    ///
    /// This operation ignores any existing clipping and transforations.
    ///
    /// # Note:
    ///
    /// You probably don't want to call this. It is essentially a specialized
    /// fill method that can be used in GUI contexts for things like clearing
    /// damage regions. It does not have a good cross-platform implementation,
    /// and eventually should be deprecated when support is added for blend
    /// modes, at which point it will be easier to just use [`fill`] for
    /// everything.
    ///
    /// [`fill`]: #method.fill
    fn clear(&mut self, region: impl Into<Option<Rect>>, color: Color);

    /// Stroke a [`Shape`], using the default [`StrokeStyle`].
    fn stroke(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>, width: f64);

    /// Stroke a [`Shape`], providing a custom [`StrokeStyle`].
    fn stroke_styled(
        &mut self,
        shape: impl Shape,
        brush: &impl IntoBrush<Self>,
        width: f64,
        style: &StrokeStyle,
    );

    /// Fill a [`Shape`], using the [non-zero fill rule].
    ///
    /// [non-zero fill rule]: https://en.wikipedia.org/wiki/Nonzero-rule
    fn fill(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>);

    /// Fill a shape, using the [even-odd fill rule].
    ///
    /// [even-odd fill rule]: https://en.wikipedia.org/wiki/Even–odd_rule
    fn fill_even_odd(&mut self, shape: impl Shape, brush: &impl IntoBrush<Self>);

    /// Clip to a [`Shape`].
    ///
    /// All subsequent drawing operations up to the next [`restore`](#method.restore)
    /// are clipped by the shape.
    fn clip(&mut self, shape: impl Shape);

    /// Returns a reference to a shared [`Text`] object.
    ///
    /// This provides access to the text API.
    fn text(&mut self) -> &mut Self::Text;

    /// Draw a [`TextLayout`].
    ///
    /// The `pos` parameter specifies the baseline of the left starting place of
    /// the text. Note: this is true even if the text is right-to-left.
    fn draw_text(&mut self, layout: &Self::TextLayout, pos: impl Into<Point>);

    /// Save the context state.
    ///
    /// Pushes the current context state onto a stack, to be popped by
    /// [`restore`](#method.restore).
    ///
    /// Prefer [`with_save`](#method.with_save) if possible, as that statically
    /// enforces balance of save/restore pairs.
    ///
    /// The context state currently consists of a clip region and an affine
    /// transform, but is expected to grow in the near future.
    fn save(&mut self) -> Result<(), Error>;

    /// Restore the context state.
    ///
    /// Pop a context state that was pushed by [`save`](#method.save). See
    /// that method for details.
    fn restore(&mut self) -> Result<(), Error>;

    /// Do graphics operations with the context state saved and then restored.
    ///
    /// Equivalent to [`save`](#method.save), calling `f`, then
    /// [`restore`](#method.restore). See those methods for more details.
    fn with_save(&mut self, f: impl FnOnce(&mut Self) -> Result<(), Error>) -> Result<(), Error> {
        self.save()?;
        // Always try to restore the stack, even if `f` errored.
        f(self).and(self.restore())
    }

    /// Finish any pending operations.
    ///
    /// This will generally be called by a shell after all user drawing
    /// operations but before presenting. Not all back-ends will handle this
    /// the same way.
    fn finish(&mut self) -> Result<(), Error>;

    /// Apply a transform.
    ///
    /// Apply an affine transformation. The transformation remains in effect
    /// until a [`restore`](#method.restore) operation.
    fn transform(&mut self, transform: Affine);

    /// Create a new [`Image`] from a pixel buffer.
    ///
    /// This takes raw pixel data and attempts create an object that the
    /// platform knows how to draw.
    ///
    /// The generated image can be cached and reused. This is a good idea for
    /// images that are used frequently, because creating the image type may
    /// be expensive.
    ///
    /// To draw the generated image, pass it to [`draw_image`]. To draw a portion
    /// of the image, use [`draw_image_area`].
    ///
    /// If you are trying to create an image from the contents of this
    /// [`RenderContext`], see [`capture_image_area`].
    ///
    /// [`draw_image`]: #method.draw_image
    /// [`draw_image_area`]: #method.draw_image_area
    /// [`capture_image_area`]: #method.capture_image_area
    fn make_image(
        &mut self,
        width: usize,
        height: usize,
        buf: &[u8],
        format: ImageFormat,
    ) -> Result<Self::Image, Error>;

    /// Draw an [`Image`] into the provided [`Rect`].
    ///
    /// The image is scaled to fit the provided [`Rect`]; it will be squashed
    /// if the aspect ratios don't match.
    fn draw_image(
        &mut self,
        image: &Self::Image,
        dst_rect: impl Into<Rect>,
        interp: InterpolationMode,
    );

    /// Draw a specified area of an [`Image`].
    ///
    /// The `src_rect` area of `image` is scaled to the provided `dst_rect`.
    /// It will be squashed if the aspect ratios don't match.
    fn draw_image_area(
        &mut self,
        image: &Self::Image,
        src_rect: impl Into<Rect>,
        dst_rect: impl Into<Rect>,
        interp: InterpolationMode,
    );

    /// Create an [`Image`] of the specified region of the context.
    ///
    /// The `src_rect` area of the current render context will be captured
    /// as a copy and returned.
    ///
    /// This can be used for things like caching expensive drawing operations.
    fn capture_image_area(&mut self, src_rect: impl Into<Rect>) -> Result<Self::Image, Error>;

    /// Draw a rectangle with Gaussian blur.
    ///
    /// The blur radius is sometimes referred to as the "standard deviation" of
    /// the blur.
    fn blurred_rect(&mut self, rect: Rect, blur_radius: f64, brush: &impl IntoBrush<Self>);

    /// Returns the transformations currently applied to the context.
    fn current_transform(&self) -> Affine;
}

/// A trait for various types that can be used as brushes.
///
/// These include backend-independent types such `Color` and `LinearGradient`,
/// as well as the types used to represent these on a specific backend.
///
/// This is an internal trait that you should not have to implement or think about.
pub trait IntoBrush<P: RenderContext>
where
    P: ?Sized,
{
    #[doc(hidden)]
    fn make_brush<'a>(&'a self, piet: &mut P, bbox: impl FnOnce() -> Rect) -> Cow<'a, P::Brush>;
}

impl<P: RenderContext> IntoBrush<P> for Color {
    fn make_brush<'a>(&'a self, piet: &mut P, _bbox: impl FnOnce() -> Rect) -> Cow<'a, P::Brush> {
        Cow::Owned(piet.solid_brush(self.to_owned()))
    }
}

/// A color or a gradient.
///
/// This type is provided as a convenience, so that library consumers can
/// easily write methods and types that use or reference *something* that can
/// be used as a brush, without needing to know what it is.
///
/// # Examples
///
/// ```no_run
/// use piet::{Color, PaintBrush, RadialGradient};
/// use piet::kurbo::Rect;
///
/// struct Widget {
/// frame: Rect,
/// background: PaintBrush,
/// }
///
/// fn make_widget<T: Into<PaintBrush>>(frame: Rect, bg: T) -> Widget {
///     Widget {
///         frame,
///         background: bg.into(),
///     }
/// }
///
/// let color_widget = make_widget(Rect::ZERO, Color::BLACK);
/// let rad_grad = RadialGradient::new(0.8, (Color::WHITE, Color::BLACK));
/// let gradient_widget = make_widget(Rect::ZERO, rad_grad);
///
/// ```
#[derive(Debug, Clone)]
pub enum PaintBrush {
    /// A [`Color`].
    Color(Color),
    /// A [`LinearGradient`].
    Linear(LinearGradient),
    /// A [`RadialGradient`].
    Radial(RadialGradient),
    /// A [`FixedGradient`].
    Fixed(FixedGradient),
}

impl<P: RenderContext> IntoBrush<P> for PaintBrush {
    fn make_brush<'a>(&'a self, piet: &mut P, bbox: impl FnOnce() -> Rect) -> Cow<'a, P::Brush> {
        match self {
            PaintBrush::Color(color) => color.make_brush(piet, bbox),
            PaintBrush::Linear(linear) => linear.make_brush(piet, bbox),
            PaintBrush::Radial(radial) => radial.make_brush(piet, bbox),
            PaintBrush::Fixed(fixed) => fixed.make_brush(piet, bbox),
        }
    }
}

impl From<Color> for PaintBrush {
    fn from(src: Color) -> PaintBrush {
        PaintBrush::Color(src)
    }
}

impl From<LinearGradient> for PaintBrush {
    fn from(src: LinearGradient) -> PaintBrush {
        PaintBrush::Linear(src)
    }
}

impl From<RadialGradient> for PaintBrush {
    fn from(src: RadialGradient) -> PaintBrush {
        PaintBrush::Radial(src)
    }
}

impl From<FixedGradient> for PaintBrush {
    fn from(src: FixedGradient) -> PaintBrush {
        PaintBrush::Fixed(src)
    }
}

impl From<FixedLinearGradient> for PaintBrush {
    fn from(src: FixedLinearGradient) -> PaintBrush {
        PaintBrush::Fixed(src.into())
    }
}

impl From<FixedRadialGradient> for PaintBrush {
    fn from(src: FixedRadialGradient) -> PaintBrush {
        PaintBrush::Fixed(src.into())
    }
}
