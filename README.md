# Piet: a 2D graphics abstraction
[![Build Status](https://travis-ci.com/linebender/piet.svg?branch=master)](https://travis-ci.com/linebender/piet)

This repo holds an API for 2D graphics drawing.

The motivation for this crate is set forth in this [blog post]. Ideally it will become a layer to help [druid] become cross-platform.

This repo is structured as a core API crate, "piet" and a separate crate for each back-end, currently "piet-direct2d", "piet-cairo", and "piet-web". One motivation for this structure is that additional back-ends can be written without coupling to the main crate, and clients can opt in to the back-ends they need. In addition, it's possible use multiple back-ends, which will likely be useful for testing.

A companion for Bézier path representation and geometry is [kurbo].

The piet-cairo crate depends on the cairo library, found at
https://www.cairographics.org/download/.  A simple test of the cairo
backend is to run `cargo run --example basic-cairo`, which should
produce an image file called "temp-cairo.png".

The piet-direct2d create works on Windows only.  Build with `cargo
build --all` to include it.  A simple test of the direct2d backend is
to run `cargo run --example basic`, which should produce an image
called "temp-image.png".

## Roadmap

Since the project is in its infant stages, there's not currently a set roadmap. For a good idea of what the library will eventually be capable of see [this list][resvg backend requirements] of requirements to be a backend 2D graphics library for the SVG rendering library resvg.

## Contributing

Contributions are welcome! It's in early stages, so there are lots of opportunities to fill things out.

You can find other collaborators at [xi.zulipchat.com][zulip] under the #druid stream.

## Inspirations

Piet's interface is largely inspired by the [Skia Graphics Library] as well as the [C++ 2D graphics api proposal] although piet aims to be much more lightweight and modular.

## The Name

The library is of course named after [Piet Mondrian]. It's abstract and hopefully will be used for drawing lots of rectangles.

[blog post]: https://raphlinus.github.io/rust/graphics/2018/10/11/2d-graphics.html
[druid]: https://github.com/xi-editor/druid
[kurbo]: https://github.com/linebender/kurbo
[resvg backend requirements]: https://github.com/RazrFalcon/resvg/blob/master/docs/backend_requirements.md
[zulip]: https://xi.zulipchat.com
[Skia Graphics Library]: https://skia.org
[C++ 2D graphics api proposal]: http://www.open-std.org/jtc1/sc22/wg21/docs/papers/2018/p0267r8.pdf
[Piet Mondrian]: https://en.wikipedia.org/wiki/Piet_Mondrian
