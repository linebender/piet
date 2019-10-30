//! Basic example of rendering in the browser

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{console, window, HtmlCanvasElement};

use piet::RenderContext;
use piet_web::WebRenderContext;

use piet_test::draw_test_picture;

#[wasm_bindgen]
pub fn run() {
    let window = window().unwrap();
    let canvas = window
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

    let dpr = window.device_pixel_ratio();
    canvas.set_width((canvas.offset_width() as f64 * dpr) as u32);
    canvas.set_height((canvas.offset_height() as f64 * dpr) as u32);
    let _ = context.scale(dpr, dpr);

    let mut piet_context = WebRenderContext::new(&mut context, &window);
    run_tests(&mut piet_context);

    // TODO: make the test picture selectable
    draw_test_picture(&mut piet_context, 0).unwrap();
    piet_context.finish().unwrap();
}

fn run_tests(ctx: &mut WebRenderContext) {
    console::log_1(&"tests starting...".into());

    test::test_hit_test_text_position_basic(ctx);
    console::log_1(&"test hit_test_text_position_basic complete".into());

    test::test_hit_test_text_position_complex_0(ctx);
    console::log_1(&"test hit_test_text_position_complex_0 complete".into());

    test::test_hit_test_text_position_complex_1(ctx);
    console::log_1(&"test hit_test_text_position_complex_1 complete".into());

    test::test_hit_test_point_basic(ctx);
    console::log_1(&"test hit_test_point_basic complete".into());

    test::test_hit_test_point_complex(ctx);
    console::log_1(&"test hit_test_point_complex complete".into());
}

mod test {
    use crate::*;
    use piet::kurbo::Point;
    use piet::{FontBuilder, Text, TextLayout, TextLayoutBuilder};
    use web_sys::console;

    // - x: calculated value
    // - target: f64
    // - tolerance: in f64
    fn assert_close_to(x: f64, target: f64, tolerance: f64) {
        let min = target - tolerance;
        let max = target + tolerance;
        println!("x: {}, target: {}", x, target);
        assert!(x <= max && x >= min);
    }

    pub fn test_hit_test_text_position_basic(ctx: &mut WebRenderContext) {
        let text_layout = ctx;

        let input = "piet text!";
        let font = text_layout.new_font_by_name("Segoe UI", 12.0).build().unwrap();

        let layout = text_layout.new_text_layout(&font, &input[0..4]).build().unwrap();
        let piet_width = layout.width();

        let layout = text_layout.new_text_layout(&font, &input[0..3]).build().unwrap();
        let pie_width = layout.width();

        let layout = text_layout.new_text_layout(&font, &input[0..2]).build().unwrap();
        let pi_width = layout.width();

        let layout = text_layout.new_text_layout(&font, &input[0..1]).build().unwrap();
        let p_width = layout.width();

        let layout = text_layout.new_text_layout(&font, "").build().unwrap();
        let null_width = layout.width();

        let full_layout = text_layout.new_text_layout(&font, input).build().unwrap();
        let full_width = full_layout.width();

        assert_close_to(full_layout.hit_test_text_position(4, false).unwrap().point.x as f64, piet_width, 3.0);
        assert_close_to(full_layout.hit_test_text_position(3, false).unwrap().point.x as f64, pie_width, 3.0);
        assert_close_to(full_layout.hit_test_text_position(2, false).unwrap().point.x as f64, pi_width, 3.0);
        assert_close_to(full_layout.hit_test_text_position(1, false).unwrap().point.x as f64, p_width, 3.0);
        assert_close_to(full_layout.hit_test_text_position(0, false).unwrap().point.x as f64, null_width, 3.0);
        assert_close_to(full_layout.hit_test_text_position(10, false).unwrap().point.x as f64, full_width, 3.0);

        // until BIDI, trailing_hit = true will always return None
        assert_eq!(full_layout.hit_test_text_position(0, true).map(|p| p.point.x as f64), None);
    }

    pub fn test_hit_test_text_position_complex_0(ctx: &mut WebRenderContext) {
        let input = "é";
        assert_eq!(input.len(), 2);

        let text_layout = ctx;
        let font = text_layout.new_font_by_name("Segoe UI", 12.0).build().unwrap();
        let layout = text_layout.new_text_layout(&font, input).build().unwrap();

        assert_close_to(layout.hit_test_text_position(0, false).unwrap().point.x, 0.0, 3.0);
        assert_close_to(layout.hit_test_text_position(2, false).unwrap().point.x, layout.width(), 3.0);

        // note code unit not at grapheme boundary
        assert_close_to(layout.hit_test_text_position(1, false).unwrap().point.x, layout.width(), 3.0);
        assert_eq!(layout.hit_test_text_position(1, false).unwrap().metrics.text_position, 2);

        // unicode segmentation is wrong on this one for now.
        //let input = "🤦\u{1f3fc}\u{200d}\u{2642}\u{fe0f}";

        //let mut text_layout = D2DText::new();
        //let font = text_layout.new_font_by_name("Segoe UI", 12.0).build().unwrap();
        //let layout = text_layout.new_text_layout(&font, input).build().unwrap();

        //assert_eq!(input.graphemes(true).count(), 1);
        //assert_eq!(layout.hit_test_text_position(0, true).map(|p| p.point_x as f64), Some(layout.width()));
        //assert_eq!(input.len(), 17);

        let input = "\u{0023}\u{FE0F}\u{20E3}"; // #️⃣
        assert_eq!(input.len(), 7);
        assert_eq!(input.chars().count(), 3);

        let font = text_layout.new_font_by_name("Segoe UI", 12.0).build().unwrap();
        let layout = text_layout.new_text_layout(&font, input).build().unwrap();

        assert_close_to(layout.hit_test_text_position(0, false).unwrap().point.x, 0.0, 3.0);
        assert_close_to(layout.hit_test_text_position(7, false).unwrap().point.x, layout.width(), 3.0);

        // note code unit not at grapheme boundary
        assert_close_to(layout.hit_test_text_position(1, false).unwrap().point.x, layout.width(), 3.0);
        assert_eq!(layout.hit_test_text_position(1, false).unwrap().metrics.text_position, 7);
    }

    pub fn test_hit_test_text_position_complex_1(ctx: &mut WebRenderContext) {
        // Notes on this input:
        // 6 code points
        // 7 utf-16 code units (1/1/1/1/1/2)
        // 14 utf-8 code units (2/1/3/3/1/4)
        // 4 graphemes
        let input = "é\u{0023}\u{FE0F}\u{20E3}1\u{1D407}"; // #️⃣,, 𝐇
        assert_eq!(input.len(), 14);

        let text_layout = ctx;
        let font = text_layout.new_font_by_name("Segoe UI", 12.0).build().unwrap();
        let layout = text_layout.new_text_layout(&font, input).build().unwrap();

        let test_layout_0 = text_layout.new_text_layout(&font, &input[0..2]).build().unwrap();
        let test_layout_1 = text_layout.new_text_layout(&font, &input[0..9]).build().unwrap();
        let test_layout_2 = text_layout.new_text_layout(&font, &input[0..10]).build().unwrap();

        // Note: text position is in terms of utf8 code units
        assert_close_to(layout.hit_test_text_position(0, false).unwrap().point.x, 0.0, 3.0);
        assert_close_to(layout.hit_test_text_position(2, false).unwrap().point.x, test_layout_0.width(), 3.0);
        assert_close_to(layout.hit_test_text_position(9, false).unwrap().point.x, test_layout_1.width(), 3.0);
        assert_close_to(layout.hit_test_text_position(10, false).unwrap().point.x, test_layout_2.width(), 3.0);
        assert_close_to(layout.hit_test_text_position(14, false).unwrap().point.x, layout.width(), 3.0);

        // note code unit not at grapheme boundary
        assert_close_to(layout.hit_test_text_position(1, false).unwrap().point.x, test_layout_0.width(), 3.0);
        assert_eq!(layout.hit_test_text_position(1, false).unwrap().metrics.text_position, 2);
    }

    // NOTE brittle test
    pub fn test_hit_test_point_basic(ctx: &mut WebRenderContext) {
        let text_layout = ctx;

        let font = text_layout.new_font_by_name("Segoe UI", 12.0).build().unwrap();
        let layout = text_layout.new_text_layout(&font, "piet text!").build().unwrap();
        console::log_1(&format!("text pos 4: {:?}", layout.hit_test_text_position(4, false)).into()); // 23.432432174682617
        console::log_1(&format!("text pos 5: {:?}", layout.hit_test_text_position(5, false)).into()); // 27.243244171142578

        // test hit test point
        // all inside
        let pt = layout.hit_test_point(Point::new(22.5, 0.0));
        assert_eq!(pt.metrics.text_position, 4);
        let pt = layout.hit_test_point(Point::new(23.0, 0.0));
        assert_eq!(pt.metrics.text_position, 4);
        let pt = layout.hit_test_point(Point::new(25.0, 0.0));
        assert_eq!(pt.metrics.text_position, 4);
        let pt = layout.hit_test_point(Point::new(26.0, 0.0));
        assert_eq!(pt.metrics.text_position, 5);
        let pt = layout.hit_test_point(Point::new(27.0, 0.0));
        assert_eq!(pt.metrics.text_position, 5);
        let pt = layout.hit_test_point(Point::new(28.0, 0.0));
        assert_eq!(pt.metrics.text_position, 5);

        // outside
        console::log_1(&format!("layout_width: {:?}", layout.width()).into()); // 55.5405387878418

        let pt = layout.hit_test_point(Point::new(55.0, 0.0));
        assert_eq!(pt.metrics.text_position, 10); // last text position
        assert_eq!(pt.is_inside, true);

        let pt = layout.hit_test_point(Point::new(57.0, 0.0));
        assert_eq!(pt.metrics.text_position, 10); // last text position
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(-1.0, 0.0));
        assert_eq!(pt.metrics.text_position, 0); // first text position
        assert_eq!(pt.is_inside, false);
    }

    // NOTE brittle test
    pub fn test_hit_test_point_complex(ctx: &mut WebRenderContext) {
        // Notes on this input:
        // 6 code points
        // 7 utf-16 code units (1/1/1/1/1/2)
        // 14 utf-8 code units (2/1/3/3/1/4)
        // 4 graphemes
        let input = "é\u{0023}\u{FE0F}\u{20E3}1\u{1D407}"; // #️⃣,, 𝐇

        let text_layout = ctx;
        let font = text_layout.new_font_by_name("Segoe UI", 12.0).build().unwrap();
        let layout = text_layout.new_text_layout(&font, input).build().unwrap();
        console::log_1(&format!("text pos 2: {:?}", layout.hit_test_text_position(2, false)).into()); // 7.108108043670654
        console::log_1(&format!("text pos 9: {:?}", layout.hit_test_text_position(9, false)).into()); // 19.29729652404785
        console::log_1(&format!("text pos 10: {:?}", layout.hit_test_text_position(10, false)).into()); // 26.91891860961914
        console::log_1(&format!("text pos 14: {:?}", layout.hit_test_text_position(14, false)).into()); // 38.27027130126953, line width

        let pt = layout.hit_test_point(Point::new(2.0, 0.0));
        assert_eq!(pt.metrics.text_position, 0);
        let pt = layout.hit_test_point(Point::new(4.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
        let pt = layout.hit_test_point(Point::new(7.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
        let pt = layout.hit_test_point(Point::new(10.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
        let pt = layout.hit_test_point(Point::new(14.0, 0.0));
        assert_eq!(pt.metrics.text_position, 9);
        let pt = layout.hit_test_point(Point::new(18.0, 0.0));
        assert_eq!(pt.metrics.text_position, 9);
        let pt = layout.hit_test_point(Point::new(23.0, 0.0));
        assert_eq!(pt.metrics.text_position, 9);
        let pt = layout.hit_test_point(Point::new(26.0, 0.0));
        assert_eq!(pt.metrics.text_position, 10);
        let pt = layout.hit_test_point(Point::new(29.0, 0.0));
        assert_eq!(pt.metrics.text_position, 10);
        let pt = layout.hit_test_point(Point::new(32.0, 0.0));
        assert_eq!(pt.metrics.text_position, 10);
        let pt = layout.hit_test_point(Point::new(35.5, 0.0));
        assert_eq!(pt.metrics.text_position, 14);
        let pt = layout.hit_test_point(Point::new(38.0, 0.0));
        assert_eq!(pt.metrics.text_position, 14);
        let pt = layout.hit_test_point(Point::new(40.0, 0.0));
        assert_eq!(pt.metrics.text_position, 14);
    }
}
