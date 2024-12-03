use cpu_sparse::CsRenderCtx;
use piet_next::peniko::color::palette;
use piet_next::RenderCtx;
use piet_next::peniko::kurbo::BezPath;

pub fn main() {
    let mut ctx = CsRenderCtx::new(1024, 256);
    let mut path = BezPath::new();
    path.move_to((10.0, 10.0));
    path.line_to((900.0, 20.0));
    path.line_to((150.0, 200.0));
    path.close_path();
    ctx.fill(&path.into(), palette::css::REBECCA_PURPLE.into());
    ctx.debug_dump();
}
