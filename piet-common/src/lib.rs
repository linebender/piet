//! A piet backend appropriate for the current platform.
//!
//! This crate reexports the [piet crate][piet], alongside an appropriate backend
//! for the given platform. It also exposes [kurbo][], which defines shape and
//! curve types useful in drawing.
//!
//! The intention of this crate is to provide a single dependency that handles
//! the common piet use-case. If you have more complicated needs (such as
//! supporting multiple backends simultaneously) you should use crates such as
//! [piet][] and [piet-cairo][] directly.
//!
//! The associated types for brushes, text, and images are exported as type
//! definitions (resolving to concrete types within the backend), so they can
//! be used directly. The text-related types are prefixed with "Piet" to avoid
//! conflict with the text traits that would otherwise have the same name.
//!
//! Also note that all public types for the specific backend are re-exported,
//! but have their docs hidden here. These types can be useful for platform
//! integration, and also potentially to access extensions specific to the
//! backend. The types documented below can be used portable across all
//! backends.
//!
//! [piet]: https://crates.io/crates/piet
//! [kurbo]: https://crates.io/crates/kurbo
//! [piet-cairo]: https://crates.io/crates/piet-cairo

#![deny(clippy::trivially_copy_pass_by_ref)]

pub use piet::*;

#[doc(hidden)]
pub use piet::kurbo;

cfg_if::cfg_if! {
     if #[cfg(any(feature = "web", target_arch = "wasm32"))] {
        #[path = "web_back.rs"]
        mod backend;
    } else if #[cfg(all(target_os = "linux", feature = "skia"))] {
        #[path = "skia_back.rs"]
        mod backend;
    } else if #[cfg(target_os = "linux")] { 
        #[path = "cairo_back.rs"]
        mod backend;
    }
    else if #[cfg(target_os = "macos")] {
        #[path = "cg_back.rs"]
        mod backend;
    } else if #[cfg(target_os = "windows")] {
        #[path = "direct2d_back.rs"]
        mod backend;
    } else {
        compile_error!("could not select an appropriate backend");
    }
}

pub use backend::*;

#[cfg(test)]
mod test {
    use super::*;

    use static_assertions as sa;

    // Make sure all the common types exist and don't get accidentally removed
    #[allow(dead_code)]
    struct Types<'a> {
        piet: Piet<'a>,
        brush: Brush,
        piet_text: PietText,
        piet_text_layout: PietTextLayout,
        piet_text_layout_builder: PietTextLayoutBuilder,
        image: Image,
    }

    sa::assert_impl_all!(Device: Send);
    sa::assert_not_impl_any!(Device: Sync);
}
