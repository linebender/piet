use piet::RenderContext;

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
pub fn draw_evcxr(f: impl FnOnce(&mut crate::RenderContext)) -> impl evcxr_runtime::Display {
    let mut ctx = crate::RenderContext::new();
    f(&mut ctx);
    ctx.finish().unwrap();
    ctx
}
