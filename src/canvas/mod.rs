//! Infinite canvas — pan, zoom, select, drag, connect, and create note cards.

pub mod camera;
pub mod card;
pub mod connector;
pub mod markdown_preview;
pub mod tools;

use crate::model::{
    CanvasConnector, WorkspaceBoard, WorkspaceCanvasDocument,
    AGENTIC_CANVAS_ENGINE, AGENTIC_CANVAS_VERSION,
};
use crate::theme::color32;
use crate::theme::palette;
use camera::Camera;
use card::{apply_resize_handle, ensure_card_dimensions, render_card, ResizeHandle};
use connector::{render_connector, render_connection_preview};
use egui::{Pos2, Ui, Vec2};
use std::collections::HashMap;
use tools::CanvasTool;

const SAVE_DEBOUNCE_SECS: f32 = 2.0;

/// Copy `note_id` from persisted board rows when older tldraw JSON omitted it.
fn hydrate_note_ids_from_board_objects(
    doc: &mut WorkspaceCanvasDocument,
    board: &WorkspaceBoard,
) {
    for row in &board.objects {
        if let Some(obj) = doc.objects.get_mut(&row.object_id) {
            if obj.note_id.is_none() {
                obj.note_id = row.note_id.clone();
            }
        }
    }
}

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
    pub active_resize: Option<(String, ResizeHandle)>,
    pub save_in_flight: bool,
    pub drag_mutation_started: bool,
    pub show_grid: bool,
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
            active_resize: None,
            save_in_flight: false,
            drag_mutation_started: false,
            show_grid: true,
        }
    }

    pub fn load_board(&mut self, board: &WorkspaceBoard) {
        self.board_id = Some(board.board_id.clone());

        let mut doc: WorkspaceCanvasDocument =
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

        hydrate_note_ids_from_board_objects(&mut doc, board);
        for obj in doc.objects.values_mut() {
            ensure_card_dimensions(obj);
        }

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
    pub create_text_at: Option<Pos2>,
    pub delete_selection: bool,
    pub open_note_id: Option<String>,
    pub dirty: bool,
}

// ═══════════════════════════════════════════════════════════════════════════
// show()
// ═══════════════════════════════════════════════════════════════════════════

pub fn show(ui: &mut Ui, state: &mut CanvasState) -> CanvasOutput {
    let mut output = CanvasOutput::default();

    let panel_rect = ui.available_rect_before_wrap();

    let mut doc = match state.canvas_doc.take() {
        Some(doc) => doc,
        None => {
            render_floating_toolbar(ui.ctx(), panel_rect, state, &mut output, false);
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() / 2.0 - 30.0);
                ui.label(
                    egui::RichText::new("Select a board under Boards in the sidebar")
                        .color(ui.visuals().weak_text_color()),
                );
                ui.label(
                    egui::RichText::new(
                        "Pan: middle-drag or Space+drag. Zoom: Ctrl+scroll. Note tool: click the canvas.",
                    )
                    .small()
                    .weak(),
                );
            });
            return output;
        }
    };

    let mut canvas_output = CanvasOutput::default();

    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(panel_rect), |ui| {
        let rect = ui.max_rect();
        canvas_output = paint_canvas(ui, state, &mut doc, rect);
    });

    render_floating_toolbar(
        ui.ctx(),
        panel_rect,
        state,
        &mut output,
        true,
    );
    output.merge_from(&canvas_output);

    doc.camera = state.camera.to_model_camera();
    state.canvas_doc = Some(doc);

    if state.save_debounce_elapsed() {
        queue_save(state, &mut output);
    }

    output
}

impl CanvasOutput {
    fn merge_from(&mut self, other: &CanvasOutput) {
        if other.save_board.is_some() {
            self.save_board = other.save_board.clone();
        }
        if other.create_note_at.is_some() {
            self.create_note_at = other.create_note_at;
        }
        if other.create_text_at.is_some() {
            self.create_text_at = other.create_text_at;
        }
        if other.delete_selection {
            self.delete_selection = true;
        }
        if other.open_note_id.is_some() {
            self.open_note_id = other.open_note_id.clone();
        }
        if other.ingest_board {
            self.ingest_board = true;
        }
        self.dirty |= other.dirty;
    }
}

fn paint_canvas(ui: &mut Ui, state: &mut CanvasState, doc: &mut WorkspaceCanvasDocument, rect: egui::Rect) -> CanvasOutput {
    let mut output = CanvasOutput::default();

    if ui.input(|i| i.modifiers.command || i.modifiers.ctrl) && ui.input(|i| i.key_pressed(egui::Key::Z))
    {
        state.undo_doc(doc);
    }
    if ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
        state.delete_selection_in_doc(doc);
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

    if state.show_grid {
        render_grid(ui, &state.camera, rect);
    }

    let mut hovered_object_id: Option<String> = None;
    let mut clicked_object_id: Option<String> = None;
    let mut double_clicked_note_id: Option<String> = None;
    let mut dragged_object_id: Option<(String, Vec2)> = None;
    let mut drag_started_on_card = false;
    let mut frame_resize: Option<(String, ResizeHandle)> = None;

    let object_ids: Vec<String> = doc.objects.keys().cloned().collect();

    for obj_id in &object_ids {
        let obj = doc.objects.get(obj_id).cloned();
        let Some(obj) = obj else { continue };

        let is_selected = state.selected_object_id.as_ref() == Some(obj_id);
        let is_hovered = hovered_object_id.as_ref() == Some(obj_id) || is_selected;

        let interaction = render_card(
            ui,
            &obj,
            &state.camera,
            rect,
            state.tool,
            is_selected,
            is_hovered,
        );

        let w = if obj.w > 0.0 { obj.w } else { crate::model::NOTE_CARD_WIDTH };
        let h = if obj.h > 0.0 { obj.h } else { crate::model::NOTE_CARD_HEIGHT };
        let screen_pos = state.camera.world_to_screen(Pos2::new(obj.x, obj.y), &rect);
        let card_rect = egui::Rect::from_min_size(
            screen_pos,
            Vec2::new(w * state.camera.zoom, h * state.camera.zoom),
        );

        if interaction.hover_resize.is_some()
            || interaction.pointer_pos.map_or(false, |p| card_rect.contains(p))
        {
            hovered_object_id = Some(obj_id.clone());
        }

        if interaction.clicked {
            clicked_object_id = Some(obj_id.clone());
            if state.tool == CanvasTool::Select {
                if let Some(handle) = interaction.hover_resize {
                    state.selected_object_id = Some(obj_id.clone());
                    state.active_resize = Some((obj_id.clone(), handle));
                    frame_resize = Some((obj_id.clone(), handle));
                }
            }
        }

        if interaction.double_clicked {
            if let Some(ref note_id) = obj.note_id {
                double_clicked_note_id = Some(note_id.clone());
            }
        }

        if let Some((handle, drag_delta)) = interaction.resize_drag {
            state.selected_object_id = Some(obj_id.clone());
            dragged_object_id = Some((obj_id.clone(), drag_delta));
            drag_started_on_card = true;
            state.active_resize = Some((obj_id.clone(), handle));
            frame_resize = Some((obj_id.clone(), handle));
        } else if let Some(drag_delta) = interaction.dragged {
            dragged_object_id = Some((obj_id.clone(), drag_delta));
            drag_started_on_card = true;
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
        render_connector(ui, conn, from_obj, to_obj, &state.camera, rect);

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
                let resize_handle = frame_resize
                    .as_ref()
                    .filter(|(id, _)| id == drag_id)
                    .map(|(_, h)| *h)
                    .or_else(|| {
                        state
                            .active_resize
                            .as_ref()
                            .filter(|(id, _)| id == drag_id)
                            .map(|(_, h)| *h)
                    });
                if let Some(handle) = resize_handle {
                    if !state.drag_mutation_started {
                        state.push_undo_snapshot(&doc);
                        state.drag_mutation_started = true;
                    }
                    if let Some(obj) = doc.objects.get_mut(drag_id) {
                        apply_resize_handle(obj, handle, delta);
                        let w = if obj.w > 0.0 {
                            obj.w
                        } else {
                            crate::model::NOTE_CARD_WIDTH
                        };
                        let h = if obj.h > 0.0 {
                            obj.h
                        } else {
                            crate::model::NOTE_CARD_HEIGHT
                        };
                        state.set_status(format!("{w:.0} × {h:.0}"));
                        state.mark_dirty();
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
        state.active_resize = None;
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

    if matches!(state.tool, CanvasTool::Note | CanvasTool::Text)
        && clicked_object_id.is_none()
    {
        let bg_clicked = ui.interact(rect, ui.id().with("canvas_bg_click"), egui::Sense::click());
        if bg_clicked.clicked() {
            if let Some(p) = pointer {
                let world = state.camera.screen_to_world(p, &rect);
                match state.tool {
                    CanvasTool::Note => {
                        output.create_note_at = Some(world);
                        state.set_status("Creating note…");
                    }
                    CanvasTool::Text => {
                        output.create_text_at = Some(world);
                    }
                    _ => {}
                }
            }
        }
    }

    if state.tool.is_shape_placeholder() && clicked_object_id.is_none() {
        let bg_clicked = ui.interact(rect, ui.id().with("canvas_bg_shape"), egui::Sense::click());
        if bg_clicked.clicked() {
            state.set_status("Shapes: use Draw tab → Excalidraw bridge (agentic_canvas is note cards only)");
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

/// Floating bottom toolbar (Excalidraw-style). Rendered in foreground so it receives clicks.
fn render_floating_toolbar(
    ctx: &egui::Context,
    canvas_rect: egui::Rect,
    state: &mut CanvasState,
    output: &mut CanvasOutput,
    board_loaded: bool,
) {
    const BAR_W: f32 = 520.0;
    const BAR_H: f32 = 46.0;
    let pos = egui::pos2(
        canvas_rect.center().x - BAR_W * 0.5,
        canvas_rect.max.y - BAR_H - 14.0,
    );

    egui::Area::new(egui::Id::new("canvas_floating_toolbar"))
        .order(egui::Order::Foreground)
        .interactable(true)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            let frame_fill = color32(palette::BG_ELEVATED);
            let stroke = egui::Stroke::new(1.0, color32(palette::BORDER));
            egui::Frame::none()
                .fill(frame_fill)
                .stroke(stroke)
                .rounding(egui::Rounding::same(14.0))
                .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                .show(ui, |ui| {
                    ui.set_width(BAR_W);
                    ui.add_enabled_ui(board_loaded, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Tools")
                                .small()
                                .color(color32(palette::TEXT_SECONDARY)),
                        );
                        ui.separator();

                        for tool in [
                            CanvasTool::Select,
                            CanvasTool::Pan,
                            CanvasTool::Connect,
                            CanvasTool::Rectangle,
                            CanvasTool::Circle,
                            CanvasTool::Diamond,
                            CanvasTool::Arrow,
                            CanvasTool::Text,
                            CanvasTool::Note,
                        ] {
                            let selected = state.tool == tool;
                            let label = if tool.is_shape_placeholder() {
                                format!("{}*", tool.label())
                            } else {
                                tool.label().to_string()
                            };
                            let tip = if tool.is_shape_placeholder() {
                                format!("{label} — shapes live on Draw / Excalidraw tab")
                            } else {
                                label.clone()
                            };
                            if ui
                                .add(
                                    egui::Button::new(&label)
                                        .min_size(egui::vec2(36.0, 28.0))
                                        .selected(selected),
                                )
                                .on_hover_text(tip)
                                .clicked()
                            {
                                state.tool = tool;
                            }
                        }

                        ui.separator();

                        let grid_on = state.show_grid;
                        if ui
                            .add(
                                egui::Button::new("#")
                                    .min_size(egui::vec2(28.0, 28.0))
                                    .selected(grid_on),
                            )
                            .on_hover_text("Toggle grid")
                            .clicked()
                        {
                            state.show_grid = !state.show_grid;
                        }

                        if ui
                            .add(egui::Button::new("🗑").min_size(egui::vec2(28.0, 28.0)))
                            .on_hover_text("Delete selection")
                            .clicked()
                        {
                            output.delete_selection = true;
                        }

                        ui.separator();

                        let save_enabled = state.dirty && !state.save_in_flight;
                        if ui
                            .add_enabled(save_enabled, egui::Button::new("Save"))
                            .clicked()
                        {
                            queue_save(state, output);
                        }
                        if ui.button("Ingest").clicked() {
                            output.ingest_board = true;
                        }
                    });
                    });
                });
        });
}

fn queue_save(state: &mut CanvasState, output: &mut CanvasOutput) {
    if let Some(ref board_id) = state.board_id {
        if let Some(saved_doc) = state.save_document() {
            output.save_board = Some((board_id.clone(), saved_doc));
            state.begin_save();
        }
    }
}

fn render_grid(ui: &mut Ui, camera: &Camera, rect: egui::Rect) {
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

/// Add a text card at a world position (synchronous).
pub fn add_text_card_at(state: &mut CanvasState, world_pos: Pos2) {
    let Some(doc) = state.canvas_doc.clone() else {
        return;
    };
    state.push_undo_snapshot(&doc);
    let updated = crate::model::add_text_card_to_canvas_document(&doc, (world_pos.x, world_pos.y));
    let selected_id = updated
        .objects
        .values()
        .last()
        .map(|o| o.id.clone());
    state.canvas_doc = Some(updated);
    if let Some(obj_id) = selected_id {
        state.selected_object_id = Some(obj_id);
    }
    state.mark_dirty();
    state.set_status("Text card added");
}

/// Delete the current selection on the loaded board.
pub fn delete_selection_on_canvas(state: &mut CanvasState) {
    let Some(mut doc) = state.canvas_doc.take() else {
        return;
    };
    state.delete_selection_in_doc(&mut doc);
    state.canvas_doc = Some(doc);
}

/// Refresh every note-backed card from the canonical note list.
pub fn sync_all_note_cards(state: &mut CanvasState, notes: &[crate::model::WorkspaceNoteDocument]) {
    for note in notes {
        sync_note_card_from_note(state, note);
    }
}

/// Refresh canvas card text from the canonical note document (after save in Notes tab).
pub fn sync_note_card_from_note(state: &mut CanvasState, note: &crate::model::WorkspaceNoteDocument) {
    let Some(mut doc) = state.canvas_doc.clone() else {
        return;
    };
    let mut changed = false;
    for obj in doc.objects.values_mut() {
        if obj.note_id.as_deref() != Some(note.note_id.as_str()) {
            continue;
        }
        obj.title = note.title.clone();
        obj.summary = note.summary.clone().unwrap_or_default();
        obj.markdown_preview = crate::model::build_markdown_preview(&note.body_markdown);
        obj.tags = note.tags.clone();
        ensure_card_dimensions(obj);
        changed = true;
    }
    if changed {
        state.canvas_doc = Some(doc);
        state.mark_dirty();
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
