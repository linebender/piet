#![allow(non_upper_case_globals)]

//! core graphics gradient support

use core_foundation::{
    array::{CFArray, CFArrayRef},
    base::{CFTypeID, TCFType},
    declare_TCFType, impl_TCFType,
};
use core_graphics::{
    base::CGFloat,
    color::{CGColor, SysCGColorRef},
    color_space::{kCGColorSpaceSRGB, CGColorSpace, CGColorSpaceRef},
    context::CGContextRef,
    geometry::CGPoint,
};

use piet::kurbo::Point;
use piet::{Color, FixedGradient, FixedLinearGradient, FixedRadialGradient, GradientStop};

//FIXME: remove all this when core-graphics 0.20.0 is released
// core-graphics does not provide a CGGradient type
pub enum CGGradientT {}
pub type CGGradientRef = *mut CGGradientT;
pub type CGGradientDrawingOptions = u32;
pub const CGGradientDrawsBeforeStartLocation: CGGradientDrawingOptions = 1;
pub const CGGradientDrawsAfterEndLocation: CGGradientDrawingOptions = 1 << 1;

declare_TCFType!(CGGradient, CGGradientRef);
impl_TCFType!(CGGradient, CGGradientRef, CGGradientGetTypeID);

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
                unsafe {
                    CGContextDrawRadialGradient(
                        ctx,
                        self.cg_grad.as_concrete_TypeRef(),
                        start_center,
                        0.0, // start_radius
                        end_center,
                        radius as CGFloat,
                        options,
                    )
                }
            }
            FixedGradient::Linear(FixedLinearGradient { start, end, .. }) => {
                let start = to_cgpoint(start);
                let end = to_cgpoint(end);
                unsafe {
                    CGContextDrawLinearGradient(
                        ctx,
                        self.cg_grad.as_concrete_TypeRef(),
                        start,
                        end,
                        options,
                    )
                }
            }
        }
    }
}

fn new_cg_gradient(stops: &[GradientStop]) -> CGGradient {
    unsafe {
        //FIXME: is this expensive enough we should be reusing it?
        let space = CGColorSpace::create_with_name(kCGColorSpaceSRGB).unwrap();
        let mut colors = Vec::<CGColor>::new();
        let mut locations = Vec::<CGFloat>::new();
        for GradientStop { pos, color } in stops {
            let (r, g, b, a) = Color::as_rgba(&color);
            let color = CGColorCreate(&*space, [r, g, b, a].as_ptr());
            let color = CGColor::wrap_under_create_rule(color);
            colors.push(color);
            locations.push(*pos as CGFloat);
        }

        let colors = CFArray::from_CFTypes(&colors);
        let gradient =
            CGGradientCreateWithColors(&*space, colors.as_concrete_TypeRef(), locations.as_ptr());

        CGGradient::wrap_under_create_rule(gradient)
    }
}

fn to_cgpoint(point: Point) -> CGPoint {
    CGPoint::new(point.x as CGFloat, point.y as CGFloat)
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGGradientGetTypeID() -> CFTypeID;
    //CGColorSpaceRef is missing repr(c).
    #[allow(improper_ctypes)]
    fn CGGradientCreateWithColors(
        space: *const CGColorSpaceRef,
        colors: CFArrayRef,
        locations: *const CGFloat,
    ) -> CGGradientRef;
    #[allow(improper_ctypes)]
    fn CGColorCreate(space: *const CGColorSpaceRef, components: *const CGFloat) -> SysCGColorRef;
    #[allow(improper_ctypes)]
    fn CGContextDrawLinearGradient(
        ctx: *mut CGContextRef,
        gradient: CGGradientRef,
        startPoint: CGPoint,
        endPoint: CGPoint,
        options: u32,
    );
    #[allow(improper_ctypes)]
    fn CGContextDrawRadialGradient(
        ctx: *mut CGContextRef,
        gradient: CGGradientRef,
        startCenter: CGPoint,
        startRadius: CGFloat,
        endCenter: CGPoint,
        endRadius: CGFloat,
        options: u32,
    );
}
