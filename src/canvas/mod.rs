//! Infinite canvas — pan, zoom, select, drag, connect, and create note cards.
//!
//! The canvas is an infinite 2D workspace where cards (note cards, text cards,
//! graph references) can be placed, dragged, and connected with directional
//! edges.  The viewport is controlled by a [`Camera`] that maps between
//! screen and world space.

pub mod camera;
pub mod card;
pub mod connector;
pub mod tools;

use crate::model::{
    CanvasConnector, WorkspaceBoard, WorkspaceCanvasDocument,
    AGENTIC_CANVAS_ENGINE, AGENTIC_CANVAS_VERSION,
};
use crate::theme::color32;
use crate::theme::palette;
use camera::Camera;
use card::render_card;
use connector::{render_connector, render_connection_preview};
use egui::{Pos2, Ui, Vec2};
use std::collections::HashMap;
use tools::CanvasTool;

// ═══════════════════════════════════════════════════════════════════════════
// CanvasState
// ═══════════════════════════════════════════════════════════════════════════

/// Mutable state for the infinite canvas.
pub struct CanvasState {
    /// Viewport camera (pan + zoom).
    pub camera: Camera,
    /// Currently active tool.
    pub tool: CanvasTool,
    /// ID of the selected card (if any).
    pub selected_object_id: Option<String>,
    /// Loaded board document (cards + connectors).
    pub canvas_doc: Option<WorkspaceCanvasDocument>,
    /// ID of the currently loaded board.
    pub board_id: Option<String>,
    /// True while the user is middle-mouse or space-drag panning.
    pub is_panning: bool,
    /// Pointer position from the previous frame (screen space).
    pub last_pointer_pos: Option<Pos2>,
    /// Pointer position where a drag started (screen space).
    pub drag_start: Option<Pos2>,
    /// Object ID we started a connection drag from.
    pub connecting_from: Option<String>,
    /// True when the document has unsaved changes.
    pub dirty: bool,
    /// Transient status message with remaining display time.
    pub status_message: Option<(String, f32)>,
}

impl Default for CanvasState {
    fn default() -> Self {
        Self::new()
    }
}

impl CanvasState {
    /// Create a new canvas with default camera and no board loaded.
    pub fn new() -> Self {
        Self {
            camera: Camera::new(),
            tool: CanvasTool::default(),
            selected_object_id: None,
            canvas_doc: None,
            board_id: None,
            is_panning: false,
            last_pointer_pos: None,
            drag_start: None,
            connecting_from: None,
            dirty: false,
            status_message: None,
        }
    }

    /// Load a [`WorkspaceBoard`] into the canvas.
    ///
    /// Attempts to parse the board's `tldraw_document` JSON into a
    /// [`WorkspaceCanvasDocument`].  If parsing fails, an empty document
    /// is created.
    pub fn load_board(&mut self, board: &WorkspaceBoard) {
        self.board_id = Some(board.board_id.clone());

        let doc: WorkspaceCanvasDocument =
            serde_json::from_value(board.tldraw_document.clone()).unwrap_or_else(|_| {
                WorkspaceCanvasDocument {
                    engine: AGENTIC_CANVAS_ENGINE.to_string(),
                    version: AGENTIC_CANVAS_VERSION,
                    camera: crate::model::CanvasCamera {
                        x: 0.0,
                        y: 0.0,
                        zoom: 1.0,
                    },
                    objects: HashMap::new(),
                    connectors: HashMap::new(),
                }
            });

        // Sync camera from document
        self.camera = Camera::from_model_camera(&doc.camera);
        self.canvas_doc = Some(doc);
        self.selected_object_id = None;
        self.dirty = false;
    }

    /// Serialize the current canvas state into a [`WorkspaceCanvasDocument`].
    ///
    /// Returns `None` if no board is loaded.
    pub fn save_document(&self) -> Option<WorkspaceCanvasDocument> {
        self.canvas_doc.as_ref().map(|doc| {
            let mut doc = doc.clone();
            doc.camera = self.camera.to_model_camera();
            doc
        })
    }

    /// Fade the status message over time.
    ///
    /// Call once per frame with the frame delta time.
    pub fn tick_status(&mut self, dt: f32) {
        if let Some((_, ref mut remaining)) = self.status_message {
            *remaining -= dt;
            if *remaining <= 0.0 {
                self.status_message = None;
            }
        }
    }

    /// Show a transient status message.
    fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some((msg.into(), 3.0));
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// CanvasOutput
// ═══════════════════════════════════════════════════════════════════════════

/// Actions produced by the canvas UI.
#[derive(Debug, Default)]
pub struct CanvasOutput {
    /// Request to save the board with this document.
    pub save_board: Option<(String, WorkspaceCanvasDocument)>,
    /// Request to create a new note at this world position.
    pub create_note_at: Option<Pos2>,
    /// Board document was modified (dirty flag).
    pub dirty: bool,
}

// ═══════════════════════════════════════════════════════════════════════════
// show() — main entry point
// ═══════════════════════════════════════════════════════════════════════════

/// Render the infinite canvas and process all interactions.
///
/// Returns a [`CanvasOutput`] describing actions the caller should perform
/// (save requests, note creation, etc.).
pub fn show(ui: &mut Ui, state: &mut CanvasState) -> CanvasOutput {
    let mut output = CanvasOutput::default();
    let rect = ui.max_rect();

    // ── 0. No board loaded — show placeholder ─────────────────────────
    let Some(ref mut doc) = state.canvas_doc else {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() / 2.0 - 10.0);
            ui.label(
                egui::RichText::new("Select a board from the sidebar")
                    .color(ui.visuals().weak_text_color()),
            );
        });
        return output;
    };

    // ── 1. Background input (pan, zoom) ──────────────────────────────
    let pointer = ui.input(|i| i.pointer.hover_pos());
    let scroll_delta = ui.input(|i| i.raw_scroll_delta);
    let ctrl_down = ui.input(|i| i.modifiers.ctrl);
    let space_down = ui.input(|i| i.key_down(egui::Key::Space));
    let middle_down = ui.input(|i| i.pointer.button_down(egui::PointerButton::Middle));
    let primary_released = ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary));

    // Ctrl + wheel → zoom at cursor
    if ctrl_down && scroll_delta.y != 0.0 {
        let factor = if scroll_delta.y > 0.0 { 1.1 } else { 0.9 };
        let anchor = pointer.unwrap_or(rect.center());
        state.camera.zoom_at(factor, anchor, &rect);
    }

    // Middle-mouse drag or Space+drag → pan
    if middle_down || (space_down && state.is_panning) {
        if let Some(prev) = state.last_pointer_pos {
            if let Some(curr) = pointer {
                let delta = curr - prev;
                state.camera.pan(delta);
            }
        }
        state.is_panning = true;
    } else if !space_down {
        state.is_panning = false;
    }

    // Pan tool — left-drag on background pans
    if state.tool == CanvasTool::Pan {
        let bg_sense = ui.interact(rect, ui.id().with("canvas_bg"), egui::Sense::drag());
        if bg_sense.dragged() {
            state.camera.pan(bg_sense.drag_delta());
        }
    }

    // ── 2. Render grid ────────────────────────────────────────────────
    render_grid(ui, &state.camera);

    // ── 3. Collect card interactions ──────────────────────────────────
    let mut hovered_object_id: Option<String> = None;
    let mut clicked_object_id: Option<String> = None;
    let mut dragged_object_id: Option<(String, Vec2)> = None;
    let mut drag_started_on_card = false;

    let object_ids: Vec<String> = doc.objects.keys().cloned().collect();

    for obj_id in &object_ids {
        let obj = doc.objects.get(obj_id).cloned();
        let Some(obj) = obj else { continue };

        let is_selected = state.selected_object_id.as_ref() == Some(obj_id);
        let is_hovered = state.selected_object_id.as_ref() == Some(obj_id);

        let interaction = render_card(ui, &obj, &state.camera, is_selected, is_hovered);

        if interaction.hover_edge.is_some() || interaction.pointer_pos.map_or(false, |p| {
            let w = if obj.w > 0.0 { obj.w } else { crate::model::NOTE_CARD_WIDTH };
            let h = if obj.h > 0.0 { obj.h } else { crate::model::NOTE_CARD_HEIGHT };
            let screen_pos = state.camera.world_to_screen(Pos2::new(obj.x, obj.y), &rect);
            let card_rect = egui::Rect::from_min_size(screen_pos, Vec2::new(w * state.camera.zoom, h * state.camera.zoom));
            card_rect.contains(p)
        }) {
            hovered_object_id = Some(obj_id.clone());
        }

        if interaction.clicked {
            clicked_object_id = Some(obj_id.clone());
        }

        if let Some(drag_delta) = interaction.dragged {
            dragged_object_id = Some((obj_id.clone(), drag_delta));
            drag_started_on_card = true;
        }
    }

    // ── 4. Render connectors ──────────────────────────────────────────
    let connector_ids: Vec<String> = doc.connectors.keys().cloned().collect();
    for conn_id in &connector_ids {
        let conn = doc.connectors.get(conn_id).cloned();
        let Some(ref conn) = conn else { continue };
        let Some(ref from_obj) = doc.objects.get(&conn.from_object_id).cloned() else { continue };
        let Some(ref to_obj) = doc.objects.get(&conn.to_object_id).cloned() else { continue };
        render_connector(ui, conn, from_obj, to_obj, &state.camera);
    }

    // ── 5. Handle card interactions ───────────────────────────────────

    // Click → select (Select tool) or start connect (Connect tool)
    if let Some(ref clicked_id) = clicked_object_id {
        match state.tool {
            CanvasTool::Select => {
                state.selected_object_id = Some(clicked_id.clone());
            }
            CanvasTool::Connect => {
                state.connecting_from = Some(clicked_id.clone());
                state.drag_start = pointer;
            }
            _ => {}
        }
    }

    // Drag → move card (Select tool) or preview connection (Connect tool)
    if let Some((ref drag_id, delta)) = dragged_object_id {
        match state.tool {
            CanvasTool::Select => {
                if let Some(obj) = doc.objects.get_mut(drag_id) {
                    obj.x += delta.x;
                    obj.y += delta.y;
                    state.dirty = true;
                }
            }
            CanvasTool::Connect => {
                // Connection drag — draw preview, handled below
            }
            _ => {}
        }
    }

    // ── 6. Handle Connect tool drag & release ─────────────────────────
    if state.tool == CanvasTool::Connect {
        if let Some(ref from_id) = state.connecting_from {
            if drag_started_on_card || state.drag_start.is_some() {
                // Draw preview from source card to current pointer
                if let Some(ref from_obj) = doc.objects.get(from_id) {
                    let from_w = if from_obj.w > 0.0 {
                        from_obj.w
                    } else {
                        crate::model::NOTE_CARD_WIDTH
                    };
                    let from_h = if from_obj.h > 0.0 {
                        from_obj.h
                    } else {
                        crate::model::NOTE_CARD_HEIGHT
                    };
                    let from_screen = state
                        .camera
                        .world_to_screen(Pos2::new(from_obj.x, from_obj.y), &rect);
                    let from_rect = egui::Rect::from_min_size(
                        from_screen,
                        Vec2::new(from_w * state.camera.zoom, from_h * state.camera.zoom),
                    );
                    let from_mid = from_rect.center();

                    let to_screen = pointer.unwrap_or(from_mid);
                    render_connection_preview(ui, from_mid, to_screen, &state.camera);
                }
            }

            // On release — if hovering a target card, create connector
            if primary_released {
                if let Some(ref hover_id) = hovered_object_id {
                    if hover_id != from_id {
                        let new_conn = CanvasConnector {
                            id: uuid::Uuid::new_v4().to_string(),
                            from_object_id: from_id.clone(),
                            to_object_id: hover_id.clone(),
                            relation_intent: "related_to".to_string(),
                            label: String::new(),
                        };
                        doc.connectors.insert(new_conn.id.clone(), new_conn);
                        state.dirty = true;
                        state.set_status("Connector created");
                    }
                }
                state.connecting_from = None;
                state.drag_start = None;
            }
        }
    }

    // ── 7. Handle Note tool click on empty space ──────────────────────
    if state.tool == CanvasTool::Note && clicked_object_id.is_none() {
        let bg_clicked = ui.interact(rect, ui.id().with("canvas_bg_click"), egui::Sense::click());
        if bg_clicked.clicked() {
            if let Some(p) = pointer {
                let world_pos = state.camera.screen_to_world(p, &rect);
                output.create_note_at = Some(world_pos);
                state.set_status("Creating note…");
            }
        }
    }

    // ── 8. Background click to deselect (Select tool) ─────────────────
    if state.tool == CanvasTool::Select && clicked_object_id.is_none() && !drag_started_on_card {
        let bg_click = ui.interact(rect, ui.id().with("canvas_bg_deselect"), egui::Sense::click());
        if bg_click.clicked() {
            state.selected_object_id = None;
        }
    }

    // ── 9. Update last pointer position ───────────────────────────────
    state.last_pointer_pos = pointer;

    // ── 10. Build output ──────────────────────────────────────────────
    output.dirty = state.dirty;
    if let Some(ref board_id) = state.board_id {
        if state.dirty {
            if let Some(saved_doc) = state.save_document() {
                output.save_board = Some((board_id.clone(), saved_doc));
            }
        }
    }

    // ── 11. Status message ────────────────────────────────────────────
    if let Some((ref msg, _)) = state.status_message {
        let status_rect = rect.intersect(egui::Rect::from_min_size(
            rect.left_bottom() - Vec2::new(0.0, 30.0),
            Vec2::new(rect.width(), 30.0),
        ));
        ui.painter().text(
            status_rect.left_center() + Vec2::new(12.0, 0.0),
            egui::Align2::LEFT_CENTER,
            msg,
            egui::FontId::proportional(12.0),
            color32(palette::TEXT_SECONDARY),
        );
    }

    output
}

// ═══════════════════════════════════════════════════════════════════════════
// Grid rendering
// ═══════════════════════════════════════════════════════════════════════════

/// Render a subtle dotted-grid pattern in world space.
///
/// Dots are placed at 50 px intervals in world space.  Only dots that fall
/// inside the visible viewport are drawn.
fn render_grid(ui: &mut Ui, camera: &Camera) {
    let rect = ui.max_rect();
    let grid_spacing = 50.0;
    let dot_radius = 1.0;
    let dot_color = {
        let c = color32(palette::BORDER);
        egui::Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 40)
    };

    // Determine visible world bounds
    let top_left_world = camera.screen_to_world(rect.min, &rect);
    let bottom_right_world = camera.screen_to_world(rect.max, &rect);

    let x_start = (top_left_world.x / grid_spacing).floor() as i32;
    let x_end = (bottom_right_world.x / grid_spacing).ceil() as i32;
    let y_start = (top_left_world.y / grid_spacing).floor() as i32;
    let y_end = (bottom_right_world.y / grid_spacing).ceil() as i32;

    for gx in x_start..=x_end {
        for gy in y_start..=y_end {
            let world_pos = Pos2::new(gx as f32 * grid_spacing, gy as f32 * grid_spacing);
            let screen_pos = camera.world_to_screen(world_pos, &rect);
            if rect.contains(screen_pos) {
                ui.painter().circle_filled(screen_pos, dot_radius, dot_color);
            }
        }
    }
}
