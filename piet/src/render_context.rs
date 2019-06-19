//! The main render context trait.

use kurbo::{Affine, Rect, Shape, Vec2};

use crate::{
    Color, Error, FillRule, Gradient, RoundFrom, RoundInto, StrokeStyle, Text, TextLayout,
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
pub enum ImageFormat {
    /// 3 bytes per pixel, in RGB order.
    Rgb,
    /// 4 bytes per pixel, in RGBA order, with separate alpha.
    RgbaSeparate,
    /// 4 bytes per pixel, in RGBA order, with premultiplied alpha.
    RgbaPremul,
    /// More formats may be added later.
    #[doc(hidden)]
    _NonExhaustive,
}

impl ImageFormat {
    pub fn bytes_per_pixel(&self) -> usize {
        match *self {
            ImageFormat::Rgb => 3,
            ImageFormat::RgbaPremul | ImageFormat::RgbaSeparate => 4,
            _ => panic!(),
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
pub trait RenderContext {
    /// The type of a 2D point, for this backend.
    ///
    /// Generally this needs to be a newtype so that the `RoundFrom` traits
    /// can be implemented on it. Possibly this can be relaxed in the future,
    /// as we move towards a standard `RoundFrom`.
    type Point: Into<Vec2> + RoundFrom<Vec2> + RoundFrom<(f32, f32)> + RoundFrom<(f64, f64)>;

    /// The type of 1D measurements, for example stroke width.
    ///
    /// Generally this will be either f32 or f64.
    type Coord: Into<f64> + RoundFrom<f64>;

    /// The type of a "brush".
    ///
    /// Initially just a solid RGBA color, but will probably expand to gradients.
    type Brush;

    /// An associated factory for creating text layouts and related resources.
    type Text: Text<TextLayout = Self::TextLayout>;
    type TextLayout: TextLayout;

    /// The associated type of an image.
    type Image;

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
    /// responsiblity? We could have a cache that is flushed when the Direct2D
    /// render target is rebuilt. Solid brushes are super lightweight, but
    /// other potentially retained objects will be heavier.
    fn solid_brush(&mut self, color: Color) -> Self::Brush;

    /// Create a new gradient brush.
    fn gradient(&mut self, gradient: Gradient) -> Result<Self::Brush, Error>;

    /// Clear the canvas with the given color.
    ///
    /// Note: only opaque colors are meaningful.
    fn clear(&mut self, color: Color);

    /// Stroke a shape.
    fn stroke(
        &mut self,
        shape: impl Shape,
        brush: &Self::Brush,
        width: impl RoundInto<Self::Coord>,
        style: Option<&StrokeStyle>,
    );

    /// Fill a shape.

    // TODO: switch last two argument order to be more similar to clip? Maybe we
    // should have a convention, geometry first.
    fn fill(&mut self, shape: impl Shape, brush: &Self::Brush, fill_rule: FillRule);

    /// Clip to a shape.
    ///
    /// All subsequent drawing operations up to the next [`restore`](#method.restore)
    /// are clipped by the shape.
    fn clip(&mut self, shape: impl Shape, fill_rule: FillRule);

    fn text(&mut self) -> &mut Self::Text;

    /// Draw a text layout.
    ///
    /// The `pos` parameter specifies the baseline of the left starting place of
    /// the text. Note: this is true even if the text is right-to-left.
    fn draw_text(
        &mut self,
        layout: &Self::TextLayout,
        pos: impl RoundInto<Self::Point>,
        brush: &Self::Brush,
    );

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

    /// Create a new image from a pixel buffer.
    fn make_image(
        &mut self,
        width: usize,
        height: usize,
        buf: &[u8],
        format: ImageFormat,
    ) -> Result<Self::Image, Error>;

    /// Draw an image.
    ///
    /// The image is scaled to the provided `rect`. It will be squashed if
    /// aspect ratios don't match.
    fn draw_image(&mut self, image: &Self::Image, rect: impl Into<Rect>, interp: InterpolationMode);
}
