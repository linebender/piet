// Copyright 2020 the Piet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(non_upper_case_globals)]

//! core graphics gradient support

use core_graphics::{
    base::CGFloat,
    color_space::CGColorSpace,
    context::CGContextRef,
    geometry::CGPoint,
    gradient::{CGGradient, CGGradientDrawingOptions},
};

use piet::kurbo::Point;
use piet::{Color, FixedGradient, FixedLinearGradient, FixedRadialGradient, GradientStop};

/// A wrapper around CGGradient
#[derive(Clone)]
pub struct Gradient {
    cg_grad: CGGradient,
    piet_grad: FixedGradient,
}

impl Gradient {
    pub(crate) fn from_piet_gradient(gradient: FixedGradient) -> Gradient {
        let cg_grad = match &gradient {
            FixedGradient::Linear(grad) => new_cg_gradient(&grad.stops),
            FixedGradient::Radial(grad) => new_cg_gradient(&grad.stops),
        };
        Gradient {
            cg_grad,
            piet_grad: gradient,
        }
    }

    pub(crate) fn fill(&self, ctx: &mut CGContextRef, options: CGGradientDrawingOptions) {
        match self.piet_grad {
            FixedGradient::Radial(FixedRadialGradient {
                center,
                origin_offset,
                radius,
                ..
            }) => {
                let start_center = to_cgpoint(center + origin_offset);
                let end_center = to_cgpoint(center);
                ctx.draw_radial_gradient(
                    &self.cg_grad,
                    start_center,
                    0.0,
                    end_center,
                    radius as CGFloat,
                    options,
                );
            }
            FixedGradient::Linear(FixedLinearGradient { start, end, .. }) => {
                let start = to_cgpoint(start);
                let end = to_cgpoint(end);
                ctx.draw_linear_gradient(&self.cg_grad, start, end, options);
            }
        }
    }
}

fn new_cg_gradient(stops: &[GradientStop]) -> CGGradient {
    //FIXME: is this expensive enough we should be reusing it?
    let space = CGColorSpace::create_device_rgb();
    let mut components = Vec::<CGFloat>::new();
    let mut locations = Vec::<CGFloat>::new();
    for GradientStop { pos, color } in stops {
        let (r, g, b, a) = Color::as_rgba(*color);
        components.extend_from_slice(&[r, g, b, a]);
        locations.push(*pos as CGFloat);
    }
    CGGradient::create_with_color_components(&space, &components, &locations, locations.len())
}

fn to_cgpoint(point: Point) -> CGPoint {
    CGPoint::new(point.x as CGFloat, point.y as CGFloat)
}
