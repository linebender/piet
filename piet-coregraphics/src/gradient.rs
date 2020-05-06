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

// core-graphics does not provide a CGGradient type
pub enum CGGradientT {}
pub type CGGradientRef = *mut CGGradientT;

declare_TCFType! {
    CGGradient, CGGradientRef
}

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

    pub(crate) fn first_color(&self) -> Color {
        match &self.piet_grad {
            FixedGradient::Linear(grad) => grad.stops.first().map(|g| g.color.clone()),
            FixedGradient::Radial(grad) => grad.stops.first().map(|g| g.color.clone()),
        }
        .unwrap_or(Color::BLACK)
    }

    pub(crate) fn fill(&self, ctx: &mut CGContextRef) {
        let context_ref: *mut u8 = ctx as *mut CGContextRef as *mut u8;
        match self.piet_grad {
            FixedGradient::Radial(FixedRadialGradient {
                center,
                origin_offset,
                radius,
                ..
            }) => {
                let start_center = to_cgpoint(center + origin_offset);
                let center = to_cgpoint(center);
                unsafe {
                    CGContextDrawRadialGradient(
                        context_ref,
                        self.cg_grad.as_concrete_TypeRef(),
                        start_center,
                        0.0,
                        center,
                        radius as CGFloat,
                        0,
                    )
                }
            }
            FixedGradient::Linear(FixedLinearGradient { start, end, .. }) => {
                let start = to_cgpoint(start);
                let end = to_cgpoint(end);
                unsafe {
                    CGContextDrawLinearGradient(
                        context_ref,
                        self.cg_grad.as_concrete_TypeRef(),
                        start,
                        end,
                        0,
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
        let space_ref: *const u8 = &*space as *const CGColorSpaceRef as *const u8;
        let mut colors = Vec::<CGColor>::new();
        let mut locations = Vec::<CGFloat>::new();
        for GradientStop { pos, color } in stops {
            let (r, g, b, a) = Color::as_rgba(&color);
            let color = CGColorCreate(space_ref as *const u8, [r, g, b, a].as_ptr());
            let color = CGColor::wrap_under_create_rule(color);
            colors.push(color);
            locations.push(*pos as CGFloat);
        }

        let colors = CFArray::from_CFTypes(&colors);
        let gradient = CGGradientCreateWithColors(
            space_ref as *const u8,
            colors.as_concrete_TypeRef(),
            locations.as_ptr(),
        );

        CGGradient::wrap_under_create_rule(gradient)
    }
}

fn to_cgpoint(point: Point) -> CGPoint {
    CGPoint::new(point.x as CGFloat, point.y as CGFloat)
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGGradientGetTypeID() -> CFTypeID;
    fn CGGradientCreateWithColors(
        space: *const u8,
        colors: CFArrayRef,
        locations: *const CGFloat,
    ) -> CGGradientRef;
    fn CGColorCreate(space: *const u8, components: *const CGFloat) -> SysCGColorRef;
    fn CGContextDrawLinearGradient(
        ctx: *mut u8,
        gradient: CGGradientRef,
        startPoint: CGPoint,
        endPoint: CGPoint,
        options: u32,
    );
    fn CGContextDrawRadialGradient(
        ctx: *mut u8,
        gradient: CGGradientRef,
        startCenter: CGPoint,
        startRadius: CGFloat,
        endCenter: CGPoint,
        endRadius: CGFloat,
        options: u32,
    );
}
