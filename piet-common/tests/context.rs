use kurbo::Affine;
use piet_common::*;

/// Whatever transform may be set on the underlying context by the platform
/// (such as for DPI scaling) should be ignored by piet.
#[test]
fn initial_transform_is_identity() {
    let mut device = Device::new().unwrap();
    let mut target = device.bitmap_target(400, 400, 2.0).unwrap();
    let mut ctx = target.render_context();
    let t = ctx.current_transform();
    ctx.finish().unwrap();
    assert_eq!(t, Affine::default());
}
