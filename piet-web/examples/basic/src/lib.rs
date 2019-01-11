//! Basic example of rendering on Cairo.

use kurbo::{Affine, BezPath, Line, Vec2};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{window, HtmlCanvasElement};

use piet::{FillRule, FontBuilder, RenderContext, TextLayout, TextLayoutBuilder};
use piet_web::WebRenderContext;

// Note: this could be a Shape.
fn star(center: Vec2, inner: f64, outer: f64, n: usize) -> BezPath {
    let mut result = BezPath::new();
    let d_th = std::f64::consts::PI / (n as f64);
    for i in 0..n {
        let outer_pt = center + outer * Vec2::from_angle(d_th * ((i * 2) as f64));
        if i == 0 {
            result.moveto(outer_pt);
        } else {
            result.lineto(outer_pt);
        }
        result.lineto(center + inner * Vec2::from_angle(d_th * ((i * 2 + 1) as f64)));
    }
    result.closepath();
    result
}

fn draw_pretty_picture<R: RenderContext>(rc: &mut R) {
    rc.clear(0xFF_FF_FF);
    let brush = rc.solid_brush(0x00_00_80_FF);
    rc.stroke(&Line::new((10.0, 10.0), (100.0, 50.0)), &brush, 1.0, None);

    let mut path = BezPath::new();
    path.moveto((50.0, 10.0));
    path.quadto((60.0, 50.0), (100.0, 90.0));
    let brush = rc.solid_brush(0x00_80_00_FF);
    rc.stroke(&path, &brush, 1.0, None);

    let mut path = BezPath::new();
    path.moveto((10.0, 20.0));
    path.curveto((10.0, 80.0), (100.0, 80.0), (100.0, 60.0));
    let brush = rc.solid_brush(0x00_00_80_C0);
    rc.fill(&path, &brush, FillRule::NonZero);

    let font = rc.new_font_by_name("Segoe UI", 12.0).build();
    let layout = rc.new_text_layout(&font, "Hello piet-web!").build();
    let w: f64 = layout.width().into();
    let brush = rc.solid_brush(0x80_00_00_C0);
    rc.draw_text(&layout, (80.0, 10.0), &brush);

    rc.stroke(
        &Line::new((80.0, 12.0), (80.0 + w, 12.0)),
        &brush,
        1.0,
        None,
    );

    rc.save();
    rc.transform(Affine::rotate(0.1));
    rc.draw_text(&layout, (80.0, 10.0), &brush);
    rc.restore();

    let clip_path = star(Vec2::new(90.0, 45.0), 10.0, 30.0, 24);
    rc.clip(&clip_path, FillRule::NonZero);
    let layout = rc.new_text_layout(&font, "Clipped text").build();
    rc.draw_text(&layout, (80.0, 50.0), &brush);
}

#[wasm_bindgen]
pub fn run() {
    let canvas = window()
        .unwrap()
        .document()
        .unwrap()
        .get_element_by_id("canvas")
        .unwrap()
        .dyn_into::<HtmlCanvasElement>()
        .unwrap();
    let mut context = canvas
        .get_context("2d")
        .unwrap()
        .unwrap()
        .dyn_into::<web_sys::CanvasRenderingContext2d>()
        .unwrap();

    let dpr = window().unwrap().device_pixel_ratio();
    canvas.set_width((canvas.offset_width() as f64 * dpr) as u32);
    canvas.set_height((canvas.offset_height() as f64 * dpr) as u32);
    let _ = context.scale(dpr, dpr);

    let mut piet_context = WebRenderContext::new(&mut context);
    draw_pretty_picture(&mut piet_context);
}
