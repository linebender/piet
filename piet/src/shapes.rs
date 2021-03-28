//! Options for drawing paths.

use std::rc::Rc;

/// Options for drawing stroked lines.
///
/// You may configure particular aspects of the style by using the
/// methods described below.
///
/// ## Defaults
///
/// Currently, the style (and its various consituent parts) have [`Default`]
/// impls that conform to the defaults described in the
/// [Postscript Language Manual, 3rd Edition][PLRMv3]; that document is the
/// basis for the choice of these types, and can be consulted for detailed
/// explanations and illustrations.
///
/// It is possible that in the future certain of these defaults may change;
/// if you are particular about your style you can create the various types
/// explicitly instead of relying on the default impls.
///
/// ```
/// use piet::{LineJoin, StrokeStyle};
///
/// const CONST_STLYE: StrokeStyle = StrokeStyle::new()
///     .dash_pattern(&[5.0, 1.0, 2.0])
///     .line_join(LineJoin::Round);
///
/// let style = StrokeStyle::new()
///     .dash_pattern(&[10.0, 5.0, 2.0])
///     .dash_offset(5.0);
///
/// ```
///
/// [PLRMv3]: https://www.adobe.com/content/dam/acom/en/devnet/actionscript/articles/PLRM.pdf
#[derive(Clone, PartialEq, Debug, Default)]
pub struct StrokeStyle {
    /// How to join segments of the path.
    ///
    /// By default, this is [`LineJoin::Miter`] with a `limit` of `10.0`.
    pub line_join: LineJoin,
    /// How to terminate open paths.
    ///
    /// (closed paths do not have ends.)
    ///
    /// by default, this is [`LineCap::Butt`].
    pub line_cap: LineCap,
    /// The sequence of alternating dashes and gaps uses to draw the line.
    ///
    /// If the sequence is not empty, all numbers should be finite and
    /// non-negative, and the sequence should not be all zeros.
    ///
    /// On platforms that do not support an odd number of lengths in the array,
    /// the implementation may concatenate two copies of the array to reach
    /// an even count.
    ///
    /// By default, this is empty (`&[]`), indicating a solid line.
    pub dash_pattern: StrokeDash,
    /// The distance into the `dash_pattern` at which drawing begins.
    ///
    /// By default, this is `0.0`.
    pub dash_offset: f64,
}

/// A type that represents an alternating pattern of drawn and undrawn segments.
///
/// We use our own type as a way of making this work in `const` contexts.
///
/// This type `Deref`s to `&[f64]`.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct StrokeDash {
    slice: &'static [f64],
    alloc: Option<Rc<[f64]>>,
}

/// Options for angled joins in strokes.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LineJoin {
    /// The outer edges of the two paths are extended until they intersect.
    ///
    /// Because the miter length can be extreme for small angles, you must supply
    /// a 'limit' at which we will fallback on [`LineJoin::Bevel`].
    ///
    /// This limit is the distance from the point where the inner edges of the
    /// stroke meet to the point where the outer edges meet.
    ///
    /// The default limit is `10.0`.
    ///
    /// This is also currently the default `LineJoin`; you should only need to
    /// construct it if you need to customize the `limit`.
    Miter {
        /// The maximum distance between the inner and outer stroke edges before beveling.
        limit: f64,
    },
    /// The two lines are joined by a circular arc.
    Round,
    /// The two segments are capped with [`LineCap::Butt`], and the notch is filled.
    Bevel,
}

impl LineJoin {
    /// The default maximum length for a [`LineJoin::Miter`].
    ///
    /// This is defined in the [Postscript Language Reference][PLRMv3] (pp 676).
    ///
    /// [PLRMv3]: https://www.adobe.com/content/dam/acom/en/devnet/actionscript/articles/PLRM.pdf
    pub const DEFAULT_MITER_LIMIT: f64 = 10.0;
}

/// Options for the cap of stroked lines.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LineCap {
    /// The stroke is squared off at the endpoint of the path.
    Butt,
    /// The stroke ends in a semicircular arc with a diameter equal to the line width.
    Round,
    /// The stroke projects past the end of the path, and is squared off.
    ///
    /// The stroke projects for a distance equal to half the width of the line.
    Square,
}

impl StrokeStyle {
    /// Create a new `StrokeStyle` with the provided pattern.
    ///
    /// For no pattern (a solid line) pass `&[]`.
    ///
    /// This is available in a `const` context and does not allocate;
    /// the other methods for setting the dash pattern *do* allocate, for
    /// annoying reasons.
    ///
    /// # Example
    ///
    /// ```
    ///  use piet::{LineJoin, StrokeStyle};
    ///
    ///  const STYLE: StrokeStyle = StrokeStyle::new()
    ///     .dash_pattern(&[4.0, 2.0])
    ///     .dash_offset(8.0)
    ///     .line_join(LineJoin::Round);
    /// ```
    pub const fn new() -> StrokeStyle {
        StrokeStyle {
            dash_pattern: StrokeDash {
                slice: &[],
                alloc: None,
            },
            line_join: LineJoin::Miter {
                limit: LineJoin::DEFAULT_MITER_LIMIT,
            },
            line_cap: LineCap::Butt,
            dash_offset: 0.0,
        }
    }

    /// Builder-style method to set the [`LineJoin`].
    ///
    /// [`LineJoin`]: enum.LineJoin.html
    pub const fn line_join(mut self, line_join: LineJoin) -> Self {
        self.line_join = line_join;
        self
    }

    /// Builder-style method to set the [`LineCap`].
    ///
    /// [`LineCap`]: enum.LineCap.html
    pub const fn line_cap(mut self, line_cap: LineCap) -> Self {
        self.line_cap = line_cap;
        self
    }

    /// Builder-style method to set the [`dash_offset`].
    ///
    /// [`dash_offset`]: StrokeStyle#structfield.dash_offset
    pub const fn dash_offset(mut self, offset: f64) -> Self {
        self.dash_offset = offset;
        self
    }

    /// Builder-style method to set the [`dash_pattern`].
    ///
    /// This method takes a `&'static [f64]`, and does not allocate. If you
    /// do not have a static slice, you may use [`set_dash_pattern`] instead,
    /// which does allocate.
    ///
    /// [`dash_pattern`]: #structfield.dash_pattern
    /// [`set_dash_pattern`]: #method.set_dash_pattern
    pub const fn dash_pattern(mut self, lengths: &'static [f64]) -> Self {
        self.dash_pattern.slice = lengths;
        self
    }

    /// Builder-style method to set the line dash.
    ///
    /// Dash style is represented as a vector of alternating on-off lengths,
    /// and an offset length.
    #[deprecated(since = "0.4.0", note = "Use dash_offset and dash_lengths instead")]
    #[doc(hidden)]
    pub fn dash(mut self, dashes: Vec<f64>, offset: f64) -> Self {
        self.dash_pattern.alloc = Some(dashes.into());
        self.dash_offset(offset)
    }

    /// Set the [`LineJoin`].
    pub fn set_line_join(&mut self, line_join: LineJoin) {
        self.line_join = line_join;
    }

    /// Set the [`LineCap`].
    pub fn set_line_cap(&mut self, line_cap: LineCap) {
        self.line_cap = line_cap;
    }

    #[deprecated(
        since = "0.4.0",
        note = "Use set_dash_offset and set_dash_pattern instead"
    )]
    #[doc(hidden)]
    pub fn set_dash(&mut self, dashes: Vec<f64>, offset: f64) {
        self.dash_offset = offset;
        self.dash_pattern.alloc = Some(dashes.into());
    }

    /// Set the dash offset.
    pub fn set_dash_offset(&mut self, offset: f64) {
        self.dash_offset = offset;
    }

    /// Set the dash pattern.
    ///
    /// This method always allocates. To construct without allocating, use the
    /// [`dash_pattern`] builder method.
    ///
    /// [`dash_pattern`]: #method.dash_pattern
    pub fn set_dash_pattern(&mut self, lengths: impl Into<Rc<[f64]>>) {
        self.dash_pattern.alloc = Some(lengths.into());
    }

    /// If the current [`LineJoin`] is [`LineJoin::Miter`] return the miter limit.
    pub fn miter_limit(&self) -> Option<f64> {
        match self.line_join {
            LineJoin::Miter { limit } => Some(limit),
            _ => None,
        }
    }
}

impl Default for LineJoin {
    fn default() -> Self {
        LineJoin::Miter {
            limit: LineJoin::DEFAULT_MITER_LIMIT,
        }
    }
}

impl Default for LineCap {
    fn default() -> Self {
        LineCap::Butt
    }
}

impl std::ops::Deref for StrokeDash {
    type Target = [f64];
    fn deref(&self) -> &Self::Target {
        if let Some(alloc) = &self.alloc {
            &alloc
        } else {
            self.slice
        }
    }
}
