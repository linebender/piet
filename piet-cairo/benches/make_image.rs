use cairo::{Context, Format, ImageSurface};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use piet::{ImageFormat, RenderContext};
use piet_cairo::CairoRenderContext;
use std::convert::{TryFrom, TryInto};

fn fill_random(data: &mut [u8]) {
    // A simple LCG with parameters from Wikipedia. See glibc/ ANSI C, CodeWarrior, ... in
    // https://en.wikipedia.org/w/index.php?title=Linear_congruential_generator&oldid=1028647893#Parameters_in_common_use
    let mut state: u32 = 123456789;
    let m: u32 = 1 << 31;
    let a: u32 = 1103515245;
    let c: u32 = 12345;

    let mut next_number = || {
        state = (a * state + c) % m;
        // Take a higher byte since it is more random than the low bytes
        (state >> 16) as u8
    };

    data.iter_mut().for_each(|b| *b = next_number());
}

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

        let mut data = vec![0; usize::try_from(bytes).expect("Should fit into usize")];
        fill_random(&mut data[..]);

        c.bench_function(&format!("make_image_{}_{:?}", name, format), |b| {
            let unused_surface =
                ImageSurface::create(Format::ARgb32, 1, 1).expect("Can't create surface");
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
