use piet::RenderContext;
use piet_direct2d::{BitmapOptions, D2DRenderContext};
use piet_test::draw_test_picture;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::windows::WindowExtWindows,
    window::WindowBuilder,
};

const HIDPI: f32 = 1.0;
fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    // Create the D2D factory
    let d2d = piet_direct2d::D2DFactory::new().unwrap();
    let dwrite = piet_direct2d::DwriteFactory::new().unwrap();

    // Initialize a D3D Device
    let (d3d, _d3d_ctx) = piet_direct2d::d3d::D3D11Device::create().unwrap();

    // Create the D2D Device and Context
    let mut device = unsafe { d2d.create_device(d3d.as_dxgi().unwrap().as_raw()).unwrap() };
    let mut context = device.create_device_context().unwrap();

    let swapchain = unsafe { d3d.create_swapchain_from_hwnd(window.hwnd() as _).unwrap() };
    let mut backbuffer = Some(swapchain.get_buffer().unwrap());
    let mut target = Some(unsafe {
        context
            .create_bitmap_from_dxgi(
                &backbuffer.as_ref().unwrap().as_dxgi(),
                HIDPI,
                BitmapOptions::TARGET | BitmapOptions::CANNOT_DRAW,
            )
            .unwrap()
    });
    context.set_target(target.as_ref());
    context.set_dpi_scale(HIDPI);

    event_loop.run(move |event, _, control_flow| {
        &device; // capture the device otherwise it won't be dropped..

        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => {
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                window_id,
            } if window_id == window.id() => {
                // Release current references to the backbuffer, resize and recreate
                context.set_target(None);
                target.take();
                backbuffer.take();

                swapchain.resize().unwrap();

                backbuffer = Some(swapchain.get_buffer().unwrap());
                target = Some(unsafe {
                    context
                        .create_bitmap_from_dxgi(
                            &backbuffer.as_ref().unwrap().as_dxgi(),
                            HIDPI,
                            BitmapOptions::TARGET | BitmapOptions::CANNOT_DRAW,
                        )
                        .unwrap()
                });
                context.set_target(target.as_ref());
            }
            Event::RedrawRequested(window_id) if window_id == window.id() => {
                context.begin_draw();

                let mut piet_context = D2DRenderContext::new(&d2d, &dwrite, &mut context);
                draw_test_picture(&mut piet_context, 1).unwrap();
                piet_context.finish().unwrap();

                context.end_draw().unwrap();

                swapchain.present().unwrap();
            }
            _ => (),
        }
    });
}
