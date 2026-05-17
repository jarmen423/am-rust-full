pub mod model;
pub mod theme;

pub use model::*;
pub use theme::*;

#[cfg(feature = "jina-ladybug-index")]
pub mod repo_jina_lb;

// ── Frontend modules (Phase 2) ─────────────────────────────────────
// Gated behind the `egui` feature so the backend binary does not
// depend on egui/eframe/ehttp.

#[cfg(feature = "egui")]
pub mod api;
#[cfg(feature = "egui")]
pub mod app;
#[cfg(feature = "egui")]
pub mod canvas;
#[cfg(feature = "egui")]
pub mod editor;
#[cfg(feature = "egui")]
pub mod sidebar;

// ── WASM entry point ─────────────────────────────────────────────────
// When the crate is built as a cdylib for trunk, this function is
// called by the WASM runtime to start the egui app.

#[cfg(all(feature = "egui", target_arch = "wasm32"))]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    use wasm_bindgen::JsCast;

    console_error_panic_hook::set_once();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async move {
        let document = web_sys::window()
            .expect("no window")
            .document()
            .expect("no document");
        let canvas = document
            .get_element_by_id("canvas")
            .expect("missing #canvas in index.html")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#canvas must be an HtmlCanvasElement");

        eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| {
                    let style = theme::agentic_style();
                    cc.egui_ctx.set_style(style);
                    Ok(Box::new(app::WorkspaceApp::new(cc)))
                }),
            )
            .await
            .expect("failed to start eframe");
    });
}
