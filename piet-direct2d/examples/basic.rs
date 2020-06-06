//! Basic example of rendering on Direct2D.

use winapi::shared::dxgi::DXGI_MAP_READ;

use piet::RenderContext;
use piet_direct2d::D2DRenderContext;

const TEXTURE_WIDTH: u32 = 400;
const TEXTURE_HEIGHT: u32 = 200;

const TEXTURE_WIDTH_S: usize = TEXTURE_WIDTH as usize;
const TEXTURE_HEIGHT_S: usize = TEXTURE_HEIGHT as usize;

const HIDPI: f32 = 2.0;

fn main() {
    let test_picture_number = std::env::args()
        .nth(1)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);

    // Create the D2D factory
    let d2d = piet_direct2d::D2DFactory::new().unwrap();
    let dwrite = piet_direct2d::DwriteFactory::new().unwrap();

    // Initialize a D3D Device
    let (d3d, d3d_ctx) = piet_direct2d::d3d::D3D11Device::create().unwrap();

    // Create the D2D Device and Context
    let mut device = unsafe { d2d.create_device(d3d.as_dxgi().unwrap().as_raw()).unwrap() };
    let mut context = device.create_device_context().unwrap();

    // Create a texture to render to
    let tex = d3d
        .create_texture(
            TEXTURE_WIDTH,
            TEXTURE_HEIGHT,
            piet_direct2d::d3d::TextureMode::Target,
        )
        .unwrap();

    // Bind the backing texture to a D2D Bitmap
    let target = unsafe {
        context
            .create_bitmap_from_dxgi(&tex.as_dxgi(), HIDPI)
            .unwrap()
    };

    context.set_target(&target);
    context.set_dpi_scale(HIDPI);
    context.begin_draw();
    let mut piet_context = D2DRenderContext::new(&d2d, dwrite, &mut context);
    // TODO: report errors more nicely than these unwraps.
    piet::draw_test_picture(&mut piet_context, test_picture_number).unwrap();
    piet_context.finish().unwrap();
    std::mem::drop(piet_context);
    context.end_draw().unwrap();

    let temp_texture = d3d
        .create_texture(
            TEXTURE_WIDTH,
            TEXTURE_HEIGHT,
            piet_direct2d::d3d::TextureMode::Read,
        )
        .unwrap();

    // Get the data so we can write it to a file
    // TODO: Have a safe way to accomplish this :D
    let mut raw_pixels: Vec<u8> = Vec::with_capacity(TEXTURE_WIDTH_S * TEXTURE_HEIGHT_S * 4);
    unsafe {
        d3d_ctx
            .inner()
            .CopyResource(temp_texture.as_raw() as *mut _, tex.as_raw() as *mut _);
        d3d_ctx.inner().Flush();

        let surface = temp_texture.as_dxgi();
        let mut mapped_rect = std::mem::zeroed();
        let _hr = surface.Map(&mut mapped_rect, DXGI_MAP_READ);
        for y in 0..TEXTURE_HEIGHT {
            let src = mapped_rect
                .pBits
                .offset(mapped_rect.Pitch as isize * y as isize);
            let dst = raw_pixels
                .as_mut_ptr()
                .offset(TEXTURE_WIDTH_S as isize * 4 * y as isize);
            std::ptr::copy_nonoverlapping(src, dst, TEXTURE_WIDTH_S * 4);
        }
        raw_pixels.set_len(TEXTURE_WIDTH_S * TEXTURE_HEIGHT_S * 4);
    }

    image::save_buffer(
        "temp-image.png",
        &raw_pixels,
        TEXTURE_WIDTH,
        TEXTURE_HEIGHT,
        image::ColorType::Rgba8,
    )
    .unwrap();
}
