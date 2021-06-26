use std::convert::{TryFrom, TryInto};
use piet::{ImageFormat, RenderContext};
use piet_cairo::CairoRenderContext;
use cairo::{Context, Format, ImageSurface};
use criterion::{black_box, criterion_group, criterion_main, Criterion};

pub fn bench_make_image(c: &mut Criterion) {
    let formats = [
        (1, ImageFormat::Grayscale),
        (3, ImageFormat::Rgb),
        (4, ImageFormat::RgbaSeparate),
        (4, ImageFormat::RgbaPremul),
    ];
    for &(bpp, format) in formats.iter() {
        let (name, width, height) = ("2160p", 3840, 2160);
        let bytes = width * height * bpp;

        let data = vec![0; usize::try_from(bytes).expect("Should fit into usize")];

        c.bench_function(&format!("make_image_{}_{:?}", name, format), |b| {
            let unused_surface = ImageSurface::create(Format::ARgb32, 1, 1).expect("Can't create surface");
            let cr = Context::new(&unused_surface);
            let mut piet_context = CairoRenderContext::new(&cr);

            let width = black_box(width.try_into().unwrap());
            let height = black_box(height.try_into().unwrap());
            let data = black_box(&data);
            let format = black_box(format);

            b.iter(|| piet_context.make_image(width, height, data, format));
        });
    }
}

criterion_group!(benches, bench_make_image);
criterion_main!(benches);
