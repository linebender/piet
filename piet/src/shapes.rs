//! Options for drawing paths.

/// A fill rule for resolving winding numbers.
#[derive(Clone, Copy, PartialEq)]
pub enum FillRule {
    /// Fill everything with a non-zero winding number.
    NonZero,
    /// Fill everything with an odd winding number.
    EvenOdd,
}

/// Options for drawing stroked lines.
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

    pub fn line_join(mut self, line_join: LineJoin) -> Self {
        self.line_join = Some(line_join);
        self
    }

    pub fn line_cap(mut self, line_cap: LineCap) -> Self {
        self.line_cap = Some(line_cap);
        self
    }

    pub fn dash(mut self, dashes: Vec<f64>, offset: f64) -> Self {
        self.dash = Some((dashes, offset));
        self
    }

    pub fn miter_limit(mut self, miter_limit: f64) -> Self {
        self.miter_limit = Some(miter_limit);
        self
    }
}
