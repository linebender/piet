use ::{
    glutin::{
        dpi::PhysicalSize,
        event::{Event, KeyboardInput, VirtualKeyCode, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
        window::WindowBuilder,
        ContextBuilder, GlProfile, GlRequest,
    },
    pathfinder_canvas::{Canvas, CanvasFontContext, Path2D},
    pathfinder_color::ColorF,
    pathfinder_geometry::{
        rect::RectF,
        vector::{vec2f, vec2i},
    },
    pathfinder_gl::{GLDevice, GLVersion},
    pathfinder_renderer::{
        concurrent::{rayon::RayonExecutor, scene_proxy::SceneProxy},
        gpu::{
            options::{DestFramebuffer, RendererOptions},
            renderer::Renderer,
        },
        options::BuildOptions,
    },
    pathfinder_resources::embedded::EmbeddedResourceLoader,
    piet::{
        kurbo::{Circle, Rect},
        Color, RenderContext,
    },
    piet_pathfinder::PfContext,
};

fn main() {
    // Calculate the right logical size of the window.
    let event_loop = EventLoop::new();
    let window_size = vec2i(640, 480);
    let physical_window_size = PhysicalSize::new(window_size.x() as f64, window_size.y() as f64);

    // Open a window.
    let window_builder = WindowBuilder::new()
        .with_title("piet-pathfinder example")
        .with_inner_size(physical_window_size);

    // Create an OpenGL 3.x context for Pathfinder to use.
    let gl_context = ContextBuilder::new()
        .with_gl(GlRequest::Latest)
        .with_gl_profile(GlProfile::Core)
        .build_windowed(window_builder, &event_loop)
        .unwrap();

    // Load OpenGL, and make the context current.
    let gl_context = unsafe { gl_context.make_current().unwrap() };
    gl::load_with(|name| gl_context.get_proc_address(name) as *const _);

    // Create a Pathfinder renderer.
    let mut renderer = Renderer::new(
        GLDevice::new(GLVersion::GL3, 0),
        &EmbeddedResourceLoader::new(),
        DestFramebuffer::full_window(window_size),
        RendererOptions {
            background_color: Some(ColorF::white()),
        },
    );

    // Make a canvas. We're going to draw a house.
    let font_context = CanvasFontContext::from_system_source();
    let mut canvas = Canvas::new(window_size.to_f32()).get_context_2d(font_context);
    use_piet(PfContext::new(&mut canvas));

    // Render the canvas to screen.
    let scene = SceneProxy::from_scene(canvas.into_canvas().into_scene(), RayonExecutor);
    scene.build_and_render(&mut renderer, BuildOptions::default());
    gl_context.swap_buffers().unwrap();

    // Wait for a keypress.
    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            }
            | Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    },
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            _ => {
                *control_flow = ControlFlow::Wait;
            }
        };
        scene.build_and_render(&mut renderer, BuildOptions::default());
        gl_context.swap_buffers().unwrap();
    })
}

fn use_piet(mut ctx: impl RenderContext) {
    let brush = ctx.solid_brush(Color::WHITE);

    // Draw walls.
    ctx.clear(Color::BLACK);
    ctx.stroke(
        Rect::from_points((150.0, 110.0), (75.0, 140.0)),
        &brush,
        10.0,
    );
    ctx.stroke(
        Circle {
            radius: 10.0,
            center: (150.0, 150.0).into(),
        },
        &brush,
        2.0,
    );
    ctx.fill(Rect::from_points((130.0, 190.0), (40.0, 60.0)), &brush);

    // Draw door.

    /*
    // Draw roof.
    let mut path = Path2D::new();
    path.move_to(vec2f(50.0, 140.0));
    path.line_to(vec2f(150.0, 60.0));
    path.line_to(vec2f(250.0, 140.0));
    path.close_path();
    canvas.stroke_path(path);
    */
    ctx.finish().unwrap();
}
