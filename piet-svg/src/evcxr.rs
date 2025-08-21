// Copyright 2021 the Piet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Helpers to make the SVG output of `piet-svg` easier to use from within `evcxr_jupyter`.
//!
//! [`evcxr`] is a Rust REPL. It also provides a Rust kernel for the [Jupyter Notebook] through
//! `evcxr_jupyter`.
//!
//! [`evcxr`]: https://github.com/google/evcxr
//! [Jupyter Notebook]: https://jupyter-notebook.readthedocs.io/en/stable/notebook.html

use piet::{RenderContext, kurbo::Size};

impl evcxr_runtime::Display for crate::RenderContext {
    fn evcxr_display(&self) {
        evcxr_runtime::mime_type("text/html").text(format!(
            r#"<div style="display:flex;justify-content:center;">{}</div>"#,
            self.display()
        ))
    }
}

/// Runs the function `f`, and displays the resulting `SVG`.
///
/// For use within `evcxr_jupyter`.
pub fn draw_evcxr(
    size: Size,
    f: impl FnOnce(&mut crate::RenderContext),
) -> impl evcxr_runtime::Display {
    let mut ctx = crate::RenderContext::new(size);
    f(&mut ctx);
    ctx.finish().unwrap();
    ctx
}
