//! Basic example of rendering on Direct2D.

use direct2d;
use direct2d::enums::BitmapOptions;
use direct2d::image::Bitmap;
use direct2d::RenderTarget;
use direct3d11;
use direct3d11::flags::{BindFlags, CreateDeviceFlags};
use direct3d11::helpers::ComWrapper;
use dxgi::flags::Format;

use kurbo::{Affine, BezPath, Line, Vec2};

use piet::{FillRule, FontBuilder, RenderContext, TextLayout, TextLayoutBuilder};
use piet_direct2d::D2DRenderContext;

const TEXTURE_WIDTH: u32 = 400;
const TEXTURE_HEIGHT: u32 = 200;

const TEXTURE_WIDTH_S: usize = TEXTURE_WIDTH as usize;
const TEXTURE_HEIGHT_S: usize = TEXTURE_HEIGHT as usize;

const HIDPI: f32 = 2.0;

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
    let layout = rc.new_text_layout(&font, "Hello piet!").build();
    let w: f64 = layout.width().into();
    let brush = rc.solid_brush(0x80_00_00_C0);
    rc.draw_text(&layout, (80.0, 10.0), &brush);

    rc.stroke(
        &Line::new((80.0, 12.0), (80.0 + w, 12.0)),
        &brush,
        1.0,
        None,
    );

    rc.with_save(|rc| {
        rc.transform(Affine::rotate(0.1));
        rc.draw_text(&layout, (80.0, 10.0), &brush);
    });

    let clip_path = star(Vec2::new(90.0, 45.0), 10.0, 30.0, 24);
    rc.clip(&clip_path, FillRule::NonZero);
    let layout = rc.new_text_layout(&font, "Clipped text").build();
    rc.draw_text(&layout, (80.0, 50.0), &brush);
}

fn main() {
    // Create the D2D factory
    let d2d = direct2d::factory::Factory::new().unwrap();
    let dwrite = directwrite::factory::Factory::new().unwrap();

    // Initialize a D3D Device
    let (_, d3d, d3d_ctx) = direct3d11::device::Device::create()
        .with_flags(CreateDeviceFlags::BGRA_SUPPORT)
        .build()
        .unwrap();

    // Create the D2D Device and Context
    let device = direct2d::Device::create(&d2d, &d3d.as_dxgi()).unwrap();
    let mut context = direct2d::DeviceContext::create(&device, false).unwrap();

    // Create a texture to render to
    let tex = direct3d11::texture2d::Texture2D::create(&d3d)
        .with_size(TEXTURE_WIDTH, TEXTURE_HEIGHT)
        .with_format(Format::R8G8B8A8Unorm)
        .with_bind_flags(BindFlags::RENDER_TARGET | BindFlags::SHADER_RESOURCE)
        .build()
        .unwrap();

    // Bind the backing texture to a D2D Bitmap
    let target = Bitmap::create(&context)
        .with_dxgi_surface(&tex.as_dxgi())
        .with_dpi(96.0 * HIDPI, 96.0 * HIDPI)
        .with_options(BitmapOptions::TARGET)
        .build()
        .unwrap();

    context.set_target(&target);
    context.set_dpi(96.0 * HIDPI, 96.0 * HIDPI);
    context.begin_draw();
    let mut piet_context = D2DRenderContext::new(&d2d, &dwrite, &mut context);
    draw_pretty_picture(&mut piet_context);
    piet_context.finish();
    context.end_draw().unwrap();

    let temp_texture = direct3d11::texture2d::Texture2D::create(&d3d)
        .with_size(TEXTURE_WIDTH, TEXTURE_HEIGHT)
        .with_format(direct3d11::flags::Format::R8G8B8A8Unorm)
        .with_bind_flags(direct3d11::flags::BindFlags::NONE)
        .with_usage(direct3d11::flags::Usage::Staging)
        .with_cpu_access_flags(direct3d11::flags::CpuAccessFlags::READ)
        .build()
        .unwrap();

    // Get the data so we can write it to a file
    // TODO: Have a safe way to accomplish this :D
    let mut raw_pixels: Vec<u8> = Vec::with_capacity(TEXTURE_WIDTH_S * TEXTURE_HEIGHT_S * 4);
    unsafe {
        let ctx = &*d3d_ctx.get_raw();
        ctx.CopyResource(temp_texture.get_raw() as *mut _, tex.get_raw() as *mut _);
        ctx.Flush();

        let surface = temp_texture.as_dxgi();
        let map = surface.map(true, false, false).unwrap();
        for y in 0..TEXTURE_HEIGHT {
            raw_pixels.extend_from_slice(&map.row(y)[..TEXTURE_WIDTH_S * 4]);
        }
    }

    image::save_buffer(
        "temp-image.png",
        &raw_pixels,
        TEXTURE_WIDTH,
        TEXTURE_HEIGHT,
        image::ColorType::RGBA(8),
    )
    .unwrap();
}
