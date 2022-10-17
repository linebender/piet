# automatic backend selection for [Piet][]

`piet-common` is a crate that chooses an appropriate implementation of the
[Piet][] 2D graphics API, for the current platform.

On Windows, the backend will be [piet-direct2d][], on macOS
[piet-coregraphics][], and on Linux, OpenBSD, FreeBSD and NetBSD [piet-cairo][].
The [piet-web][] backend can be selected with the `web` feature.

[Piet]: https://crates.io/crates/piet
[piet-direct2d]: https://crates.io/crates/piet-direct2d
[piet-cairo]: https://crates.io/crates/piet-cairo
[piet-web]: https://crates.io/crates/piet-web
[piet-coregraphics]: https://crates.io/crates/piet-coregraphics
