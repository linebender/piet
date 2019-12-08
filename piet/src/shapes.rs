//! Options for drawing paths.

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
