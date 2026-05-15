//! Main application — `WorkspaceApp` implementing `eframe::App`.
//!
//! Layout: left sidebar (250 px) | central editor (remaining).
//! On startup: fetches `/api/workspace/bootstrap` then loads the note list.

use crate::api;
use crate::canvas::{self, CanvasOutput, CanvasState};
use crate::editor::{self, EditorOutput, EditorState, SaveRequest};
use crate::sidebar::{self, SidebarOutput, SidebarState};
use am_workspace::model::{WorkspaceNoteDocument, WorkspaceBoard, NoteHistoryItem};
use egui::Context;
use parking_lot::Mutex;
use std::sync::Arc;

/// Which main view is currently visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum AppView {
    /// Note editor.
    #[default]
    Editor,
    /// Infinite canvas board.
    Canvas,
}

/// Application state shared between UI and async API callbacks.
pub struct WorkspaceApp {
    // ── Sidebar ──────────────────────────────────────────────────────
    sidebar: SidebarState,

    // ── Editor ───────────────────────────────────────────────────────
    editor: EditorState,

    // ── Data cache ───────────────────────────────────────────────────
    notes: Vec<WorkspaceNoteDocument>,
    boards: Vec<WorkspaceBoard>,

    // ── Async promises ───────────────────────────────────────────────
    notes_promise: api::SharedPromise<Vec<WorkspaceNoteDocument>>,
    create_promise: api::SharedPromise<WorkspaceNoteDocument>,
    save_promise: api::SharedPromise<WorkspaceNoteDocument>,
    load_promise: api::SharedPromise<WorkspaceNoteDocument>,
    history_promise: api::SharedPromise<Vec<NoteHistoryItem>>,
    revert_promise: api::SharedPromise<WorkspaceNoteDocument>,
    boards_promise: api::SharedPromise<Vec<WorkspaceBoard>>,

    // ── Canvas ───────────────────────────────────────────────────────
    canvas: CanvasState,
    app_view: AppView,

    // ── Board promises ───────────────────────────────────────────────
    board_load_promise: api::SharedPromise<WorkspaceBoard>,
    board_save_promise: api::SharedPromise<WorkspaceBoard>,
    board_create_promise: api::SharedPromise<WorkspaceBoard>,

    // ── Currently loaded board ───────────────────────────────────────
    current_board: Option<WorkspaceBoard>,

    // ── Flags ────────────────────────────────────────────────────────
    /// True after first frame — triggers bootstrap.
    bootstrapped: bool,
    /// Currently loading a specific note.
    loading_note_id: Option<String>,
}

impl WorkspaceApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            sidebar: SidebarState::new(),
            editor: EditorState::new(),
            notes: Vec::new(),
            boards: Vec::new(),
            notes_promise: Arc::new(Mutex::new(api::Promise::Idle)),
            create_promise: Arc::new(Mutex::new(api::Promise::Idle)),
            save_promise: Arc::new(Mutex::new(api::Promise::Idle)),
            load_promise: Arc::new(Mutex::new(api::Promise::Idle)),
            history_promise: Arc::new(Mutex::new(api::Promise::Idle)),
            revert_promise: Arc::new(Mutex::new(api::Promise::Idle)),
            boards_promise: Arc::new(Mutex::new(api::Promise::Idle)),
            canvas: CanvasState::new(),
            app_view: AppView::Editor,
            board_load_promise: Arc::new(Mutex::new(api::Promise::Idle)),
            board_save_promise: Arc::new(Mutex::new(api::Promise::Idle)),
            board_create_promise: Arc::new(Mutex::new(api::Promise::Idle)),
            current_board: None,
            bootstrapped: false,
            loading_note_id: None,
        }
    }

    /// Start loading the note list from the API.
    fn refresh_notes(&mut self, ctx: &Context) {
        api::fetch_notes(self.notes_promise.clone(), ctx);
    }

    /// Start loading a single note into the editor.
    fn load_note(&mut self, note_id: &str, ctx: &Context) {
        self.loading_note_id = Some(note_id.to_string());
        api::fetch_note(note_id, self.load_promise.clone(), ctx);
    }

    /// Start loading a single board into the canvas.
    fn load_board(&mut self, board_id: &str, ctx: &Context) {
        api::fetch_board(board_id, self.board_load_promise.clone(), ctx);
    }

    /// Start saving the current note.
    fn save_note(&mut self, req: SaveRequest, ctx: &Context) {
        if req.note_id == "new" || req.note_id.is_empty() {
            // Create new
            api::create_note(
                &req.title,
                &req.body_markdown,
                req.tags,
                self.create_promise.clone(),
                ctx,
            );
        } else {
            // Update existing
            api::update_note(
                &req.note_id,
                &req.title,
                &req.body_markdown,
                req.tags,
                self.save_promise.clone(),
                ctx,
            );
        }
    }

    /// Poll async promises and update state.
    fn poll_promises(&mut self) {
        // ── Notes list ────────────────────────────────────────────────
        {
            let mut lock = self.notes_promise.lock();
            if let Some(notes) = lock.take() {
                self.notes = notes;
                // If we have a selected note, ensure it exists in the new list
                if let Some(ref selected_id) = self.sidebar.selected_note_id {
                    if !self.notes.iter().any(|n| &n.note_id == selected_id) {
                        self.sidebar.deselect();
                        self.editor.unload();
                    }
                }
            }
        }

        // ── Single note load ──────────────────────────────────────────
        {
            let mut lock = self.load_promise.lock();
            if let Some(note) = lock.take() {
                self.loading_note_id = None;
                self.editor.load_note(&note);
                // Ensure this note is selected in sidebar
                self.sidebar.select_note(&note.note_id);
                // Update in the notes list cache
                if let Some(idx) = self.notes.iter().position(|n| n.note_id == note.note_id) {
                    self.notes[idx] = note;
                }
            }
        }

        // ── Create note ───────────────────────────────────────────────
        {
            let mut lock = self.create_promise.lock();
            if let Some(note) = lock.take() {
                self.notes.push(note.clone());
                self.sidebar.select_note(&note.note_id);
                self.editor.load_note(&note);
                self.editor.set_status("Note created.");
                self.refresh_notes_after_change();
            }
        }

        // ── Save note ─────────────────────────────────────────────────
        {
            let mut lock = self.save_promise.lock();
            if let Some(note) = lock.take() {
                self.editor.load_note(&note);
                // Update in cache
                if let Some(idx) = self.notes.iter().position(|n| n.note_id == note.note_id) {
                    self.notes[idx] = note;
                }
                self.editor.set_status("Saved.");
            }
        }

        // ── History ───────────────────────────────────────────────────
        {
            let mut lock = self.history_promise.lock();
            if let Some(items) = lock.take() {
                self.editor.history = items;
            }
        }

        // ── Revert ────────────────────────────────────────────────────
        {
            let mut lock = self.revert_promise.lock();
            if let Some(note) = lock.take() {
                self.editor.load_note(&note);
                if let Some(idx) = self.notes.iter().position(|n| n.note_id == note.note_id) {
                    self.notes[idx] = note;
                }
                self.editor.set_status("Reverted to selected revision.");
                self.refresh_notes_after_change();
            }
        }

        // ── Board load ────────────────────────────────────────────────
        {
            let mut lock = self.board_load_promise.lock();
            if let Some(board) = lock.take() {
                self.canvas.load_board(&board);
                self.current_board = Some(board);
                self.app_view = AppView::Canvas;
            }
        }

        // ── Board save ────────────────────────────────────────────────
        {
            let mut lock = self.board_save_promise.lock();
            if let Some(board) = lock.take() {
                self.current_board = Some(board.clone());
                self.canvas.dirty = false;
                self.canvas.status_message = Some(("Board saved.".to_string(), 3.0));
            }
        }

        // ── Board create ──────────────────────────────────────────────
        {
            let mut lock = self.board_create_promise.lock();
            if let Some(board) = lock.take() {
                self.boards.push(board.clone());
                self.canvas.load_board(&board);
                self.current_board = Some(board);
                self.app_view = AppView::Canvas;
                self.sidebar.select_board(
                    self.current_board.as_ref().unwrap().board_id.as_str(),
                );
            }
        }

        // ── Boards list ───────────────────────────────────────────────
        {
            let mut lock = self.boards_promise.lock();
            if let Some(boards) = lock.take() {
                self.boards = boards;
            }
        }
    }

    fn refresh_notes_after_change(&mut self) {
        // The notes_promise may be in a Ready state from the initial fetch.
        // Reset it to Idle so the next frame will re-fetch.
        let mut lock = self.notes_promise.lock();
        match *lock {
            api::Promise::Ready(_) | api::Promise::Failed(_) | api::Promise::Idle => {
                *lock = api::Promise::Idle;
            }
            api::Promise::Pending => {} // already in flight
        }
    }
}

impl eframe::App for WorkspaceApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        let dt = ctx.input(|i| i.stable_dt);

        // ── Bootstrap on first frame ──────────────────────────────────
        if !self.bootstrapped {
            self.bootstrapped = true;
            self.refresh_notes(ctx);
            api::fetch_boards(self.boards_promise.clone(), ctx);
        }

        // ── Poll async results ────────────────────────────────────────
        self.poll_promises();

        // ── Tick UI state ─────────────────────────────────────────────
        self.editor.tick_status(dt);
        self.canvas.tick_status(dt);

        // ── Auto-refresh notes if idle ────────────────────────────────
        {
            let lock = self.notes_promise.lock();
            if matches!(*lock, api::Promise::Idle) {
                drop(lock);
                self.refresh_notes(ctx);
            }
        }

        // ── Apply theme ───────────────────────────────────────────────
        #[cfg(feature = "egui")]
        {
            let style = am_workspace::theme::agentic_style();
            ctx.set_style(style);
        }

        // ── Top-level layout ──────────────────────────────────────────
        egui::SidePanel::left("sidebar")
            .default_width(280.0)
            .min_width(220.0)
            .max_width(400.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                let sidebar_out = sidebar::show(ui, &mut self.sidebar, &self.notes, &self.boards);
                self.handle_sidebar_output(sidebar_out, ctx);
                ui.add_space(8.0);
            });

        // ── Canvas or Editor ──────────────────────────────────────────
        if self.app_view == AppView::Canvas {
            egui::CentralPanel::default().show(ctx, |ui| {
                let canvas_out = canvas::show(ui, &mut self.canvas);
                self.handle_canvas_output(canvas_out, ctx);
            });
        } else {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.add_space(8.0);
                let editor_out = editor::show(ui, &mut self.editor);
                self.handle_editor_output(editor_out, ctx);
                ui.add_space(8.0);
            });
        }
    }
}

// ── Output handlers ──────────────────────────────────────────────────

impl WorkspaceApp {
    fn handle_sidebar_output(&mut self, out: SidebarOutput, ctx: &Context) {
        // Existing note handling
        if let Some(note_id) = out.selected_note_id {
            self.load_note(&note_id, ctx);
            self.app_view = AppView::Editor;
        }
        if out.deselected {
            self.editor.unload();
        }
        if let Some(title) = out.create_note_title {
            api::create_note(&title, "", Vec::new(), self.create_promise.clone(), ctx);
        }

        // Board handling
        if let Some(board_id) = out.selected_board_id {
            self.load_board(&board_id, ctx);
        }
        if out.board_deselected {
            self.app_view = AppView::Editor;
            self.canvas = CanvasState::new(); // Reset canvas
            self.current_board = None;
        }
        if let Some(title) = out.create_board_title {
            api::create_board("default", &title, self.board_create_promise.clone(), ctx);
        }
    }

    fn handle_canvas_output(&mut self, out: CanvasOutput, ctx: &Context) {
        if let Some((board_id, doc)) = out.save_board {
            let tldraw_value = serde_json::to_value(&doc).unwrap_or(serde_json::Value::Null);
            let title = self.current_board.as_ref()
                .map(|b| b.title.clone())
                .unwrap_or_default();
            api::save_board(
                &board_id,
                &title,
                &tldraw_value,
                vec![],
                vec![],
                self.board_save_promise.clone(),
                ctx,
            );
            // Sync tldraw_document to current_board
            if let Some(ref mut board) = self.current_board {
                board.tldraw_document = tldraw_value;
            }
            self.canvas.dirty = false;
        }
        if let Some(_world_pos) = out.create_note_at {
            // TODO: Create a note at the given world position
            // For now, just create a note via the existing API
            // and add a card object to the canvas
            // This would need a note-to-card conversion — stub for now
            self.canvas.status_message = Some(("Note creation: stub".to_string(), 3.0));
        }
    }

    fn handle_editor_output(&mut self, out: EditorOutput, ctx: &Context) {
        if let Some(req) = out.save_note {
            self.save_note(req, ctx);
        }
        if let Some(note_id) = out.fetch_history {
            api::fetch_note_history(&note_id, self.history_promise.clone(), ctx);
        }
        if let Some((note_id, revision)) = out.revert_request {
            api::revert_note(&note_id, &revision, self.revert_promise.clone(), ctx);
        }
    }
}
