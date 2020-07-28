//! Basic example of rendering on Direct2D.

use std::path::Path;

use winapi::shared::dxgi::DXGI_MAP_READ;

use piet::{samples, RenderContext};
use piet_direct2d::D2DRenderContext;

const HIDPI: f32 = 2.0;
const FILE_PREFIX: &str = "d2d-test-";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    samples::samples_main(run_sample)
}

fn run_sample(number: usize, base_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let sample = samples::get(number);
    let size = sample.size();

    let file_name = format!("{}{}.png", FILE_PREFIX, number);
    let path = base_dir.join(file_name);

    // Create the D2D factory
    let d2d = piet_direct2d::D2DFactory::new()?;
    let dwrite = piet_direct2d::DwriteFactory::new()?;

    // Initialize a D3D Device
    let (d3d, d3d_ctx) = piet_direct2d::d3d::D3D11Device::create()?;

    // Create the D2D Device and Context
    let mut device = unsafe { d2d.create_device(d3d.as_dxgi().unwrap().as_raw())? };
    let mut context = device.create_device_context()?;

    // Create a texture to render to
    let tex = d3d
        .create_texture(
            size.width as u32,
            size.height as u32,
            piet_direct2d::d3d::TextureMode::Target,
        )
        .unwrap();

    // Bind the backing texture to a D2D Bitmap
    let target = unsafe { context.create_bitmap_from_dxgi(&tex.as_dxgi(), HIDPI)? };

    context.set_target(&target);
    context.set_dpi_scale(HIDPI);
    context.begin_draw();
    let mut piet_context = D2DRenderContext::new(&d2d, dwrite, &mut context);
    // TODO: report errors more nicely than these unwraps.
    match sample.draw(&mut piet_context) {
        Ok(()) => (),
        Err(e) => {
            // cleanup
            piet_context.finish().unwrap();
            std::mem::drop(piet_context);
            context.end_draw().unwrap();
            return Err(e.into());
        }
    };
    piet_context.finish()?;
    std::mem::drop(piet_context);
    context.end_draw()?;

    let temp_texture = d3d.create_texture(
        size.width as u32,
        size.height as u32,
        piet_direct2d::d3d::TextureMode::Read,
    )?;

    // Get the data so we can write it to a file
    // TODO: Have a safe way to accomplish this :D
    let pixel_count = (size.width * size.height) as usize * 4;
    let mut raw_pixels: Vec<u8> = Vec::with_capacity(pixel_count);
    for _ in 0..pixel_count {
        raw_pixels.push(0);
    }
    unsafe {
        d3d_ctx
            .inner()
            .CopyResource(temp_texture.as_raw() as *mut _, tex.as_raw() as *mut _);
        d3d_ctx.inner().Flush();

        let surface = temp_texture.as_dxgi();
        let mut mapped_rect = std::mem::zeroed();
        let _hr = surface.Map(&mut mapped_rect, DXGI_MAP_READ);
        for y in 0..size.height as usize {
            let src = mapped_rect
                .pBits
                .offset(mapped_rect.Pitch as isize * y as isize);
            let dst = raw_pixels
                .as_mut_ptr()
                .offset(size.width as isize * 4 * y as isize);
            std::ptr::copy_nonoverlapping(src, dst, size.width as usize * 4);
        }
        raw_pixels.set_len(pixel_count);
    }

    image::save_buffer(
        &path,
        &raw_pixels,
        size.width as u32,
        size.height as u32,
        image::ColorType::Rgba8,
    )?;
    Ok(())
}
