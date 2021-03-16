//! Options for drawing paths.

use std::borrow::Cow;

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
    pub dash_pattern: Cow<'static, [f64]>,
    /// The distance into the `dash_pattern` at which drawing begins.
    ///
    /// By default, this is `0.0`.
    pub dash_offset: f64,
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
    Miter { limit: f64 },
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
    /// Create a new, default `StrokeStyle`.
    ///
    /// To create a `StrokeStyle` in a `const` setting, use [`StrokeStyle::new_with_pattern`].
    ///
    /// # Example
    ///
    /// ```
    ///  use piet::{LineJoin, StrokeStyle};
    ///
    ///  let pattern = vec![4.0, 8.0, 2.0, 8.0];
    ///
    ///  let style = StrokeStyle::new()
    ///     .line_join(LineJoin::Round)
    ///     .dash_pattern(pattern);
    ///
    /// ```
    pub fn new() -> StrokeStyle {
        StrokeStyle {
            line_join: Default::default(),
            line_cap: Default::default(),
            dash_offset: 0.0,
            dash_pattern: Cow::Borrowed(&[]),
        }
    }

    /// Create a `StrokeStyle` with a pattern.
    ///
    /// This constructor is `const`; if you don't want a pattern you may pass
    /// an empty slice.
    ///
    /// # Example
    ///
    /// ```
    ///  use piet::{LineCap, StrokeStyle};
    ///
    ///  const DASHED_STYLE: StrokeStyle = StrokeStyle::new_with_pattern(&[8.0, 12.0])
    ///     .dash_offset(8.0)
    ///     .line_cap(LineCap::Square);
    ///
    /// ```
    pub const fn new_with_pattern(pattern: &'static [f64]) -> Self {
        StrokeStyle {
            dash_pattern: Cow::Borrowed(pattern),
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
    /// You may provide either a `Vec<f64>` or a `&'static [f64]`.
    ///
    /// This method is not available in a const context; use
    /// [`StrokeStyle::new_with_pattern`] instead.
    ///
    /// [`dash_pattern`]: StrokeStyle#structfield.dash_pattern
    pub fn dash_pattern(mut self, lengths: impl Into<Cow<'static, [f64]>>) -> Self {
        self.dash_pattern = lengths.into();
        self
    }

    /// Builder-style method to set the line dash.
    ///
    /// Dash style is represented as a vector of alternating on-off lengths,
    /// and an offset length.
    #[deprecated(since = "0.4.0", note = "Use dash_offset and dash_lengths instead")]
    #[doc(hidden)]
    pub fn dash(self, dashes: Vec<f64>, offset: f64) -> Self {
        self.dash_pattern(dashes).dash_offset(offset)
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
        self.dash_pattern = dashes.into();
    }

    /// Set the dash offset.
    pub fn set_dash_offset(&mut self, offset: f64) {
        self.dash_offset = offset;
    }

    /// Set the dash pattern.
    pub fn set_dash_pattern(&mut self, lengths: impl Into<Cow<'static, [f64]>>) {
        self.dash_pattern = lengths.into();
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
