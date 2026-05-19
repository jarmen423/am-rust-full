//! Infinite canvas — pan, zoom, select, drag, connect, and create note cards.

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
use card::{render_card, Edge};
use connector::{render_connector, render_connection_preview};
use egui::{Pos2, Ui, Vec2};
use std::collections::HashMap;
use tools::CanvasTool;

const SAVE_DEBOUNCE_SECS: f32 = 2.0;
const MIN_CARD_SIZE: f32 = 120.0;

/// Content-only snapshot for undo (camera changes are excluded).
#[derive(Debug, Clone)]
struct CanvasUndoEntry {
    objects: HashMap<String, crate::model::CanvasObject>,
    connectors: HashMap<String, CanvasConnector>,
}

// ═══════════════════════════════════════════════════════════════════════════
// CanvasState
// ═══════════════════════════════════════════════════════════════════════════

/// Mutable state for the infinite canvas.
pub struct CanvasState {
    pub camera: Camera,
    pub tool: CanvasTool,
    pub selected_object_id: Option<String>,
    pub selected_connector_id: Option<String>,
    pub canvas_doc: Option<WorkspaceCanvasDocument>,
    pub board_id: Option<String>,
    pub is_panning: bool,
    pub last_pointer_pos: Option<Pos2>,
    pub drag_start: Option<Pos2>,
    pub connecting_from: Option<String>,
    pub dirty: bool,
    pub status_message: Option<(String, f32)>,
    undo_stack: Vec<CanvasUndoEntry>,
    pub save_debounce: f32,
    pub resize_target: Option<(String, Edge)>,
    pub save_in_flight: bool,
    pub drag_mutation_started: bool,
}

impl Default for CanvasState {
    fn default() -> Self {
        Self::new()
    }
}

impl CanvasState {
    pub fn new() -> Self {
        Self {
            camera: Camera::new(),
            tool: CanvasTool::default(),
            selected_object_id: None,
            selected_connector_id: None,
            canvas_doc: None,
            board_id: None,
            is_panning: false,
            last_pointer_pos: None,
            drag_start: None,
            connecting_from: None,
            dirty: false,
            status_message: None,
            undo_stack: Vec::new(),
            save_debounce: 0.0,
            resize_target: None,
            save_in_flight: false,
            drag_mutation_started: false,
        }
    }

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

        self.camera = Camera::from_model_camera(&doc.camera);
        self.canvas_doc = Some(doc);
        self.selected_object_id = None;
        self.selected_connector_id = None;
        self.dirty = false;
        self.undo_stack.clear();
        self.save_debounce = 0.0;
        self.save_in_flight = false;
        self.drag_mutation_started = false;
    }

    pub fn save_document(&self) -> Option<WorkspaceCanvasDocument> {
        self.canvas_doc.as_ref().map(|doc| {
            let mut doc = doc.clone();
            doc.camera = self.camera.to_model_camera();
            doc
        })
    }

    pub fn tick_status(&mut self, dt: f32) {
        if let Some((_, ref mut remaining)) = self.status_message {
            *remaining -= dt;
            if *remaining <= 0.0 {
                self.status_message = None;
            }
        }

        if self.dirty && !self.save_in_flight && self.save_debounce > 0.0 {
            self.save_debounce -= dt;
        }
    }

    pub fn save_debounce_elapsed(&self) -> bool {
        self.dirty && !self.save_in_flight && self.save_debounce <= 0.0
    }

    pub fn mark_saved(&mut self) {
        self.dirty = false;
        self.save_debounce = 0.0;
        self.save_in_flight = false;
    }

    pub fn begin_save(&mut self) {
        self.save_in_flight = true;
    }

    fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some((msg.into(), 3.0));
    }

    fn push_undo_snapshot(&mut self, doc: &WorkspaceCanvasDocument) {
        self.undo_stack.push(CanvasUndoEntry {
            objects: doc.objects.clone(),
            connectors: doc.connectors.clone(),
        });
        if self.undo_stack.len() > 50 {
            self.undo_stack.remove(0);
        }
    }

    fn undo_doc(&mut self, doc: &mut WorkspaceCanvasDocument) {
        let Some(entry) = self.undo_stack.pop() else {
            self.set_status("Nothing to undo");
            return;
        };
        doc.objects = entry.objects;
        doc.connectors = entry.connectors;
        self.mark_dirty();
        self.set_status("Undone");
    }

    fn delete_selection_in_doc(&mut self, doc: &mut WorkspaceCanvasDocument) {
        if let Some(conn_id) = self.selected_connector_id.take() {
            self.push_undo_snapshot(doc);
            doc.connectors.remove(&conn_id);
            self.mark_dirty();
            self.set_status("Connector deleted");
            return;
        }

        let Some(obj_id) = self.selected_object_id.take() else {
            return;
        };

        self.push_undo_snapshot(doc);
        doc.objects.remove(&obj_id);
        doc.connectors
            .retain(|_, c| c.from_object_id != obj_id && c.to_object_id != obj_id);
        self.mark_dirty();
        self.set_status("Card deleted");
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
        self.save_debounce = SAVE_DEBOUNCE_SECS;
    }

}

// ═══════════════════════════════════════════════════════════════════════════
// CanvasOutput
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Default)]
pub struct CanvasOutput {
    pub save_board: Option<(String, WorkspaceCanvasDocument)>,
    pub ingest_board: bool,
    pub create_note_at: Option<Pos2>,
    pub open_note_id: Option<String>,
    pub dirty: bool,
}

// ═══════════════════════════════════════════════════════════════════════════
// show()
// ═══════════════════════════════════════════════════════════════════════════

pub fn show(ui: &mut Ui, state: &mut CanvasState) -> CanvasOutput {
    let mut output = CanvasOutput::default();

    render_toolbar(ui, state, &mut output);

    let mut doc = match state.canvas_doc.take() {
        Some(doc) => doc,
        None => {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() / 2.0 - 10.0);
                ui.label(
                    egui::RichText::new("Select a board from the sidebar")
                        .color(ui.visuals().weak_text_color()),
                );
            });
            return output;
        }
    };

    let rect = ui.max_rect();

    if ui.input(|i| i.modifiers.command || i.modifiers.ctrl) && ui.input(|i| i.key_pressed(egui::Key::Z))
    {
        state.undo_doc(&mut doc);
    }
    if ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
        state.delete_selection_in_doc(&mut doc);
    }

    let pointer = ui.input(|i| i.pointer.hover_pos());
    let scroll_delta = ui.input(|i| i.raw_scroll_delta);
    let ctrl_down = ui.input(|i| i.modifiers.ctrl);
    let space_down = ui.input(|i| i.key_down(egui::Key::Space));
    let middle_down = ui.input(|i| i.pointer.button_down(egui::PointerButton::Middle));
    let primary_released = ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary));

    if ctrl_down && scroll_delta.y != 0.0 {
        let factor = if scroll_delta.y > 0.0 { 1.1 } else { 0.9 };
        let anchor = pointer.unwrap_or(rect.center());
        state.camera.zoom_at(factor, anchor, &rect);
    }

    if middle_down || (space_down && state.is_panning) {
        if let Some(prev) = state.last_pointer_pos {
            if let Some(curr) = pointer {
                state.camera.pan(curr - prev);
            }
        }
        state.is_panning = true;
    } else if !space_down {
        state.is_panning = false;
    }

    if state.tool == CanvasTool::Pan {
        let bg_sense = ui.interact(rect, ui.id().with("canvas_bg"), egui::Sense::drag());
        if bg_sense.dragged() {
            state.camera.pan(bg_sense.drag_delta());
        }
    }

    render_grid(ui, &state.camera);

    let mut hovered_object_id: Option<String> = None;
    let mut clicked_object_id: Option<String> = None;
    let mut double_clicked_note_id: Option<String> = None;
    let mut dragged_object_id: Option<(String, Vec2)> = None;
    let mut drag_started_on_card = false;
    let mut resize_edge: Option<(String, Edge)> = None;

    let object_ids: Vec<String> = doc.objects.keys().cloned().collect();

    for obj_id in &object_ids {
        let obj = doc.objects.get(obj_id).cloned();
        let Some(obj) = obj else { continue };

        let is_selected = state.selected_object_id.as_ref() == Some(obj_id);
        let is_hovered = hovered_object_id.as_ref() == Some(obj_id) || is_selected;

        let interaction = render_card(ui, &obj, &state.camera, is_selected, is_hovered);

        let w = if obj.w > 0.0 { obj.w } else { crate::model::NOTE_CARD_WIDTH };
        let h = if obj.h > 0.0 { obj.h } else { crate::model::NOTE_CARD_HEIGHT };
        let screen_pos = state.camera.world_to_screen(Pos2::new(obj.x, obj.y), &rect);
        let card_rect = egui::Rect::from_min_size(
            screen_pos,
            Vec2::new(w * state.camera.zoom, h * state.camera.zoom),
        );

        if interaction.hover_edge.is_some()
            || interaction.pointer_pos.map_or(false, |p| card_rect.contains(p))
        {
            hovered_object_id = Some(obj_id.clone());
        }

        if interaction.clicked {
            clicked_object_id = Some(obj_id.clone());
        }

        if interaction.double_clicked {
            if let Some(ref note_id) = obj.note_id {
                double_clicked_note_id = Some(note_id.clone());
            }
        }

        if let Some(drag_delta) = interaction.dragged {
            if is_selected && interaction.hover_edge.is_some() {
                resize_edge = Some((obj_id.clone(), interaction.hover_edge.unwrap()));
            } else {
                dragged_object_id = Some((obj_id.clone(), drag_delta));
                drag_started_on_card = true;
            }
        }
    }

    // Connector rendering + hit testing
    let connector_ids: Vec<String> = doc.connectors.keys().cloned().collect();
    for conn_id in &connector_ids {
        let conn = doc.connectors.get(conn_id).cloned();
        let Some(ref conn) = conn else { continue };
        let Some(ref from_obj) = doc.objects.get(&conn.from_object_id).cloned() else {
            continue;
        };
        let Some(ref to_obj) = doc.objects.get(&conn.to_object_id).cloned() else {
            continue;
        };
        render_connector(ui, conn, from_obj, to_obj, &state.camera);

        if state.tool == CanvasTool::Select {
            if let Some(p) = pointer {
                let from_screen =
                    state.camera.world_to_screen(Pos2::new(from_obj.x, from_obj.y), &rect);
                let to_screen =
                    state.camera.world_to_screen(Pos2::new(to_obj.x, to_obj.y), &rect);
                let mid = Pos2::new(
                    (from_screen.x + to_screen.x) * 0.5,
                    (from_screen.y + to_screen.y) * 0.5,
                );
                if p.distance(mid) < 8.0 {
                    let hit = ui.interact(
                        egui::Rect::from_center_size(mid, Vec2::splat(16.0)),
                        ui.id().with(format!("conn-{conn_id}")),
                        egui::Sense::click(),
                    );
                    if hit.clicked() {
                        state.selected_connector_id = Some(conn_id.clone());
                        state.selected_object_id = None;
                    }
                }
            }
        }
    }

    if let Some(ref clicked_id) = clicked_object_id {
        match state.tool {
            CanvasTool::Select => {
                state.selected_object_id = Some(clicked_id.clone());
                state.selected_connector_id = None;
            }
            CanvasTool::Connect => {
                state.connecting_from = Some(clicked_id.clone());
                state.drag_start = pointer;
            }
            _ => {}
        }
    }

    if let Some((ref drag_id, delta)) = dragged_object_id {
        match state.tool {
            CanvasTool::Select => {
                if let Some((ref obj_id, edge)) = resize_edge.or_else(|| state.resize_target.clone())
                {
                    if obj_id == drag_id {
                        if state.resize_target.is_none() {
                            state.push_undo_snapshot(&doc);
                            state.resize_target = Some((obj_id.clone(), edge));
                        }
                        if let Some(obj) = doc.objects.get_mut(drag_id) {
                            apply_resize(obj, edge, delta);
                            state.mark_dirty();
                        }
                    }
                } else if doc.objects.contains_key(drag_id) {
                    if !state.drag_mutation_started {
                        state.push_undo_snapshot(&doc);
                        state.drag_mutation_started = true;
                    }
                    if let Some(obj) = doc.objects.get_mut(drag_id) {
                        obj.x += delta.x;
                        obj.y += delta.y;
                    }
                    state.mark_dirty();
                }
            }
            CanvasTool::Connect => {}
            _ => {}
        }
    }

    if primary_released {
        state.resize_target = None;
        state.drag_start = None;
        state.drag_mutation_started = false;
    }

    if state.tool == CanvasTool::Connect {
        if let Some(ref from_id) = state.connecting_from {
            if drag_started_on_card || state.drag_start.is_some() {
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

            if primary_released {
                let from_id = state.connecting_from.clone();
                if let Some(from_id) = from_id {
                    if let Some(ref hover_id) = hovered_object_id {
                        if hover_id != &from_id {
                            state.push_undo_snapshot(&doc);
                            let new_conn = CanvasConnector {
                                id: uuid::Uuid::new_v4().to_string(),
                                from_object_id: from_id,
                                to_object_id: hover_id.clone(),
                                relation_intent: "related_to".to_string(),
                                label: String::new(),
                            };
                            doc.connectors.insert(new_conn.id.clone(), new_conn);
                            state.mark_dirty();
                            state.set_status("Connector created");
                        }
                    }
                }
                state.connecting_from = None;
                state.drag_start = None;
            }
        }
    }

    if state.tool == CanvasTool::Note && clicked_object_id.is_none() {
        let bg_clicked = ui.interact(rect, ui.id().with("canvas_bg_click"), egui::Sense::click());
        if bg_clicked.clicked() {
            if let Some(p) = pointer {
                output.create_note_at = Some(state.camera.screen_to_world(p, &rect));
                state.set_status("Creating note…");
            }
        }
    }

    if state.tool == CanvasTool::Select && clicked_object_id.is_none() && !drag_started_on_card {
        let bg_click = ui.interact(rect, ui.id().with("canvas_bg_deselect"), egui::Sense::click());
        if bg_click.clicked() {
            state.selected_object_id = None;
            state.selected_connector_id = None;
        }
    }

    state.last_pointer_pos = pointer;

    output.dirty = state.dirty;
    output.open_note_id = double_clicked_note_id;

    doc.camera = state.camera.to_model_camera();
    state.canvas_doc = Some(doc);

    if state.save_debounce_elapsed() {
        queue_save(state, &mut output);
    }

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

fn render_toolbar(ui: &mut Ui, state: &mut CanvasState, output: &mut CanvasOutput) {
    ui.horizontal(|ui| {
        ui.selectable_value(&mut state.tool, CanvasTool::Select, "Select");
        ui.selectable_value(&mut state.tool, CanvasTool::Pan, "Pan");
        ui.selectable_value(&mut state.tool, CanvasTool::Connect, "Connect");
        ui.selectable_value(&mut state.tool, CanvasTool::Note, "Note");

        ui.separator();

        let save_enabled = state.dirty && !state.save_in_flight;
        if ui
            .add_enabled(save_enabled, egui::Button::new("Save"))
            .clicked()
        {
            queue_save(state, output);
        }

        if ui.button("Ingest board").clicked() {
            output.ingest_board = true;
        }

        if state.save_in_flight {
            ui.label(egui::RichText::new("Saving…").color(color32(palette::TEXT_SECONDARY)));
        } else if state.dirty {
            ui.label(
                egui::RichText::new("● Unsaved changes")
                    .color(color32(palette::ACCENT_PRIMARY)),
            );
        } else {
            ui.label(
                egui::RichText::new("Saved")
                    .color(color32(palette::TEXT_SECONDARY)),
            );
        }
    });
    ui.separator();
}

fn queue_save(state: &mut CanvasState, output: &mut CanvasOutput) {
    if let Some(ref board_id) = state.board_id {
        if let Some(saved_doc) = state.save_document() {
            output.save_board = Some((board_id.clone(), saved_doc));
            state.begin_save();
        }
    }
}

fn apply_resize(obj: &mut crate::model::CanvasObject, edge: Edge, delta: Vec2) {
    match edge {
        Edge::Right => {
            obj.w = (obj.w + delta.x).max(MIN_CARD_SIZE);
        }
        Edge::Bottom => {
            obj.h = (obj.h + delta.y).max(MIN_CARD_SIZE);
        }
        Edge::Left => {
            let new_w = (obj.w - delta.x).max(MIN_CARD_SIZE);
            obj.x += obj.w - new_w;
            obj.w = new_w;
        }
        Edge::Top => {
            let new_h = (obj.h - delta.y).max(MIN_CARD_SIZE);
            obj.y += obj.h - new_h;
            obj.h = new_h;
        }
    }
}

fn render_grid(ui: &mut Ui, camera: &Camera) {
    let rect = ui.max_rect();
    let grid_spacing = 50.0;
    let dot_radius = 1.0;
    let dot_color = {
        let c = color32(palette::BORDER);
        egui::Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 40)
    };

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

/// Add a note card to the loaded canvas document (used after async note creation).
pub fn add_note_card_to_canvas(state: &mut CanvasState, note: &crate::model::WorkspaceNoteDocument, world_pos: Pos2) {
    let Some(doc) = state.canvas_doc.clone() else {
        return;
    };
    state.push_undo_snapshot(&doc);
    let updated = crate::model::add_note_to_canvas_document(
        &doc,
        note,
        Some((world_pos.x, world_pos.y)),
    );
    let selected_id = updated
        .objects
        .values()
        .find(|o| o.note_id.as_deref() == Some(note.note_id.as_str()))
        .map(|o| o.id.clone());
    state.canvas_doc = Some(updated);
    if let Some(obj_id) = selected_id {
        state.selected_object_id = Some(obj_id);
    }
    state.mark_dirty();
    state.set_status("Note card added");
}
