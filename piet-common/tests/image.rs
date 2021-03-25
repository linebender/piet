use kurbo::{Rect, Size};
use piet_common::*;

fn with_context(cb: impl FnOnce(&mut Piet) -> Result<(), String>) {
    let mut device = Device::new().unwrap();
    let mut target = device.bitmap_target(400, 400, 2.0).unwrap();
    let mut ctx = target.render_context();
    // We don't unwrap here because at least on Windows, dropping the context before calling
    // `finish` causes another panic, which results in an abort where you don't get any debugging
    // info about the first panic.
    let res = cb(&mut ctx);
    ctx.finish().unwrap();
    if let Err(e) = res {
        panic!("{}", e)
    }
}

#[test]
fn empty_image_should_not_panic() {
    let image = ImageBuf::empty();
    with_context(|ctx| {
        // Do it this way round so that if the backend panics on an empty image, we get a better
        // report of the panic for the reasons mentioned in `with_context`.
        let image = ctx
            .make_image(
                image.width(),
                image.height(),
                image.raw_pixels(),
                image.format(),
            )
            .map_err(|e| e.to_string())?;
        ctx.draw_image(
            &image,
            Rect::new(0., 0., 400., 400.),
            InterpolationMode::Bilinear,
        );
        Ok(())
    })
}

#[test]
fn empty_image_dest_should_not_panic() {
    let image = ImageBuf::from_raw(&[0, 0, 0, 0][..], ImageFormat::RgbaSeparate, 1, 1);
    with_context(|ctx| {
        let image = image.to_image(ctx);
        ctx.draw_image(
            &image,
            Rect::new(0., 0., 0., 0.),
            InterpolationMode::Bilinear,
        );
        Ok(())
    })
}

#[test]
fn empty_image_area_should_not_panic() {
    let image = ImageBuf::from_raw(&[0, 0, 0, 0][..], ImageFormat::RgbaSeparate, 1, 1);
    with_context(|ctx| {
        let image = image.to_image(ctx);
        ctx.draw_image_area(
            &image,
            Rect::new(0., 0., 0., 0.),
            Rect::new(0., 0., 1., 1.),
            InterpolationMode::Bilinear,
        );
        Ok(())
    })
}

#[test]
fn image_size() {
    let image = ImageBuf::from_raw(&[0, 0, 0, 0][..], ImageFormat::RgbaSeparate, 1, 1);
    with_context(|ctx| {
        let image = image.to_image(ctx);
        if image.size() != Size::new(1., 1.) {
            return Err(format!(
                "expected {:?}, found {:?}",
                Size::new(1., 1.),
                image.size()
            ));
        }
        Ok(())
    });

    // try an empty image
    let image = ImageBuf::empty();
    with_context(|ctx| {
        let image = image.to_image(ctx);
        if image.size() != Size::new(0., 0.) {
            return Err(format!(
                "expected {:?}, found {:?}",
                Size::new(0., 0.),
                image.size()
            ));
        }
        Ok(())
    });
}
