//! Options for drawing paths.

/// Options for drawing stroked lines.
/// Most of these are self explanatory, but some aren't.
///
/// `dash` has two parts. The Vec<f64> pattern array and an offset. The array
/// represents alternating lengths to be drawn and undrawn repeatedly. The offset
/// specifes how far into the pattern it should start. On platforms that do not
/// support an odd number of lengths in the array, the implementation may
/// concatenate two copies of the array to reach an even count.
///
/// `miter_limit` controls how corners are drawn when `line_join` is set to
/// Miter. Will draw corners as `Bevel` instead of `Miter` if the limit is
/// reached. See the reference below on how `miter_limit` is calculated.
///
/// See
/// https://www.adobe.com/content/dam/acom/en/devnet/actionscript/articles/psrefman.pdf
/// for more information and examples
#[derive(Clone, PartialEq, Debug)]
pub struct StrokeStyle {
    pub line_join: Option<LineJoin>,
    pub line_cap: Option<LineCap>,
    pub dash: Option<(Vec<f64>, f64)>,
    pub miter_limit: Option<f64>,
}

/// Options for angled joins in strokes.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

/// Options for the cap of stroked lines.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

impl StrokeStyle {
    pub fn new() -> StrokeStyle {
        StrokeStyle {
            line_join: None,
            line_cap: None,
            dash: None,
            miter_limit: None,
        }
    }

    pub fn set_line_join(&mut self, line_join: LineJoin) {
        self.line_join = Some(line_join);
    }

    pub fn set_line_cap(&mut self, line_cap: LineCap) {
        self.line_cap = Some(line_cap);
    }

    pub fn set_dash(&mut self, dashes: Vec<f64>, offset: f64) {
        self.dash = Some((dashes, offset));
    }

    pub fn set_miter_limit(&mut self, miter_limit: f64) {
        self.miter_limit = Some(miter_limit);
    }
}
