// Copyright 2024 the Piet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Utilities for flattening

use flatten::stroke::LoweredPath;
use piet_next::peniko::color::PremulRgba8;
use piet_next::peniko::kurbo::{self, BezPath, Line, Point, Stroke};
use piet_next::peniko::Color;

/// A container for line soup.
#[derive(Default)]
pub struct SoupBowl {
    pub lines: Vec<LineSoup>,
    pub colors: Vec<PremulRgba8>,
}

use crate::tiling::LineSoup;

/// The flattening tolerance
const TOL: f64 = 0.25;

impl SoupBowl {
    pub fn fill(&mut self, path: &BezPath, scale: f64, color: Color) {
        let tol = TOL / scale;
        let path_ix = self.colors.len() as u32;
        let mut start = Point::default();
        let mut p0 = Point::default();
        kurbo::flatten(path, tol, |el| match el {
            kurbo::PathEl::MoveTo(p) => {
                start = p;
                p0 = p;
            }
            kurbo::PathEl::LineTo(p) => {
                let pt0 = [(p0.x * scale) as f32, (p0.y * scale) as f32];
                let pt1 = [(p.x * scale) as f32, (p.y * scale) as f32];
                self.lines.push(LineSoup::new(path_ix, pt0, pt1));
                p0 = p;
            }
            kurbo::PathEl::QuadTo(_, _1) => todo!(),
            kurbo::PathEl::CurveTo(_, _1, _2) => todo!(),
            kurbo::PathEl::ClosePath => {
                let pt0 = [(p0.x * scale) as f32, (p0.y * scale) as f32];
                let pt1 = [(start.x * scale) as f32, (start.y * scale) as f32];
                if pt0 != pt1 {
                    self.lines.push(LineSoup::new(path_ix, pt0, pt1));
                }
            }
        });
        self.colors.push(color.premultiply().to_rgba8());
    }

    pub fn stroke(&mut self, path: &BezPath, style: &Stroke, scale: f64, color: Color) {
        let tol = TOL / scale;
        let path_ix = self.colors.len() as u32;
        let lines: LoweredPath<Line> = flatten::stroke::stroke_undashed(path, style, tol);
        for line in &lines.path {
            let p0 = [(line.p0.x * scale) as f32, (line.p0.y * scale) as f32];
            let p1 = [(line.p1.x * scale) as f32, (line.p1.y * scale) as f32];
            self.lines.push(LineSoup::new(path_ix, p0, p1));
        }

        self.colors.push(color.premultiply().to_rgba8());
    }
}
