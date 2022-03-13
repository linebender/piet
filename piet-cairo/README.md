# piet-cairo: Cairo backend for piet

This is the [Cairo](https://www.cairographics.org/) back-end for the piet graphics API.

## Toy text API

For simplicity, the back-end currently uses the [toy text API] in Cairo. Essentially, this means there is no shaping, so complex scripts won't render correctly at all, and Latin will be missing kerning, ligatures, and other refinements. According to the docs, "Any serious application should avoid them."

Fairly soon, I hope to have some type of higher-level text in place. One possibility is [pango]. From what I can tell, this should work well on Linux, but since it has a non-optional glib dependency, it might be non-trivial to get it building portably. It's also not clear to me how well this approach handles discovering system fonts.

Another possibility is to use HarfBuzz more directly, using the [rust-harfbuzz] bindings. This will require more work for font discovery and selection, but has the possibility to be considerably more native. A good Rust-native candidate for system font discovery is [font-kit].

A third possibility is to adapt [libTXT] from Flutter. This is a state of the art text layout library, with considerable investment in making it work well on mobile. However, it is in C++ and thus at the very least will need nontrivial work to make good Rust bindings.

The need for text shaping will be common to many low-level renderers that are not supported by system text services, not just Cairo.

## Building on non-Linux

Cairo is quite portable, and it is quite feasible to build on other systems. However, the [cairo-rs] crate seems to expect a library to be provided, rather than building it from sources.

On Windows, I've been using prebuilt binary releases from [cairo-windows].

On macOS with Homebrew, the following should work:

```shell
brew install cairo
```

On OpenBSD, the library can be installed from official packages:
```shell
pkg_add cairo
```

On FreeBSD, the library can be installed with `pkg`:
```shell
pkg install cairo
```

A pkg-config file is provided as usual and cairo-rs will build as expected.

TODO: nicer installation instructions (contributions welcome)

[Cairo]: https://www.cairographics.org/
[toy text API]: https://cairographics.org/manual/cairo-text.html#cairo-text.description
[cairo-rs]: https://crates.io/crates/cairo-rs
[cairo-windows]: https://github.com/preshing/cairo-windows
[pango]: https://github.com/gtk-rs/pango
[rust-harfbuzz]: https://github.com/servo/rust-harfbuzz
[libTXT]: https://github.com/flutter/flutter/issues/11092
[Gtk-rs requirements]: http://gtk-rs.org/docs/requirements.html
[font-kit]: https://github.com/pcwalton/font-kit
