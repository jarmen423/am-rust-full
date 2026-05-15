//! Frontend entry point for native (desktop) builds.
//!
//! For WASM, the entry point is the `#[wasm_bindgen(start)]` function in `lib.rs`.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use eframe::NativeOptions;

    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("Agentic Memory Workspace"),
        ..Default::default()
    };

    eframe::run_native(
        "Agentic Memory Workspace",
        options,
        Box::new(|cc| {
            // Apply the Agentic Memory theme
            #[cfg(feature = "egui")]
            {
                let style = am_workspace::theme::agentic_style();
                cc.egui_ctx.set_style(style);
            }
            Ok(Box::new(am_workspace::app::WorkspaceApp::new(cc)))
        }),
    )
    .expect("failed to start eframe");
}

#[cfg(target_arch = "wasm32")]
fn main() {
    // WASM entry point is the #[wasm_bindgen(start)] function in lib.rs.
    // This binary is not used on WASM.
}
