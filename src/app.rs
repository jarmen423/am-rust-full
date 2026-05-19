//! Main application — `WorkspaceApp` implementing `eframe::App`.

use crate::api;
use crate::canvas::{self, CanvasOutput, CanvasState};
use crate::editor::{self, EditorOutput, EditorState, SaveRequest};
use crate::graph_view::{self, GraphViewState};
use crate::model::{
    derive_workspace_artifacts_from_canvas_document, NoteHistoryItem, WorkspaceBoard,
    WorkspaceNoteDocument,
};
use crate::sidebar::{self, SidebarOutput, SidebarState};
use egui::{Context, Pos2};
use parking_lot::Mutex;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum AppView {
    #[default]
    Editor,
    Canvas,
    Graph,
}

pub struct WorkspaceApp {
    sidebar: SidebarState,
    editor: EditorState,
    notes: Vec<WorkspaceNoteDocument>,
    boards: Vec<WorkspaceBoard>,
    notes_promise: api::SharedPromise<Vec<WorkspaceNoteDocument>>,
    create_promise: api::SharedPromise<WorkspaceNoteDocument>,
    save_promise: api::SharedPromise<WorkspaceNoteDocument>,
    load_promise: api::SharedPromise<WorkspaceNoteDocument>,
    history_promise: api::SharedPromise<Vec<NoteHistoryItem>>,
    revert_promise: api::SharedPromise<WorkspaceNoteDocument>,
    boards_promise: api::SharedPromise<Vec<WorkspaceBoard>>,
    canvas: CanvasState,
    graph: GraphViewState,
    app_view: AppView,
    board_load_promise: api::SharedPromise<WorkspaceBoard>,
    board_save_promise: api::SharedPromise<WorkspaceBoard>,
    board_create_promise: api::SharedPromise<WorkspaceBoard>,
    graph_promise: api::SharedPromise<crate::model::GraphResponse>,
    current_board: Option<WorkspaceBoard>,
    bootstrapped: bool,
    loading_note_id: Option<String>,
    pending_note_world_pos: Option<Pos2>,
    pending_canvas_note: bool,
    graph_loaded: bool,
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
            graph: GraphViewState::default(),
            app_view: AppView::Editor,
            board_load_promise: Arc::new(Mutex::new(api::Promise::Idle)),
            board_save_promise: Arc::new(Mutex::new(api::Promise::Idle)),
            board_create_promise: Arc::new(Mutex::new(api::Promise::Idle)),
            graph_promise: Arc::new(Mutex::new(api::Promise::Idle)),
            current_board: None,
            bootstrapped: false,
            loading_note_id: None,
            pending_note_world_pos: None,
            pending_canvas_note: false,
            graph_loaded: false,
        }
    }

    fn refresh_notes(&mut self, ctx: &Context) {
        api::fetch_notes(self.notes_promise.clone(), ctx);
    }

    fn load_note(&mut self, note_id: &str, ctx: &Context) {
        self.loading_note_id = Some(note_id.to_string());
        api::fetch_note(note_id, self.load_promise.clone(), ctx);
    }

    fn load_board(&mut self, board_id: &str, ctx: &Context) {
        api::fetch_board(board_id, self.board_load_promise.clone(), ctx);
    }

    fn save_note(&mut self, req: SaveRequest, ctx: &Context) {
        if req.note_id == "new" || req.note_id.is_empty() {
            api::create_note(
                &req.title,
                &req.body_markdown,
                req.tags,
                self.create_promise.clone(),
                ctx,
            );
        } else {
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

    fn load_graph(&mut self, ctx: &Context) {
        api::fetch_graph_explore(self.graph_promise.clone(), ctx);
    }

    fn poll_promises(&mut self) {
        {
            let mut lock = self.notes_promise.lock();
            if let Some(notes) = lock.take() {
                self.notes = notes;
                if let Some(ref selected_id) = self.sidebar.selected_note_id {
                    if !self.notes.iter().any(|n| &n.note_id == selected_id) {
                        self.sidebar.deselect();
                        self.editor.unload();
                    }
                }
            }
        }

        {
            let mut lock = self.load_promise.lock();
            if let Some(note) = lock.take() {
                self.loading_note_id = None;
                self.editor.load_note(&note);
                self.sidebar.select_note(&note.note_id);
                if let Some(idx) = self.notes.iter().position(|n| n.note_id == note.note_id) {
                    self.notes[idx] = note;
                }
            }
        }

        {
            let note_done = {
                let mut lock = self.create_promise.lock();
                lock.take()
            };
            if let Some(note) = note_done {
                self.notes.push(note.clone());

                if self.pending_canvas_note {
                    if let Some(pos) = self.pending_note_world_pos.take() {
                        canvas::add_note_card_to_canvas(&mut self.canvas, &note, pos);
                        self.sidebar.select_note(&note.note_id);
                        self.pending_canvas_note = false;
                    }
                } else {
                    self.sidebar.select_note(&note.note_id);
                    self.editor.load_note(&note);
                    self.app_view = AppView::Editor;
                    self.editor.set_status("Note created.");
                }
                self.refresh_notes_after_change();
            }
        }

        {
            let mut lock = self.save_promise.lock();
            if let Some(note) = lock.take() {
                self.editor.load_note(&note);
                if let Some(idx) = self.notes.iter().position(|n| n.note_id == note.note_id) {
                    self.notes[idx] = note;
                }
                self.editor.set_status("Saved.");
            }
        }

        {
            let mut lock = self.history_promise.lock();
            if let Some(items) = lock.take() {
                self.editor.history = items;
            }
        }

        {
            let note_done = {
                let mut lock = self.revert_promise.lock();
                lock.take()
            };
            if let Some(note) = note_done {
                self.editor.load_note(&note);
                if let Some(idx) = self.notes.iter().position(|n| n.note_id == note.note_id) {
                    self.notes[idx] = note;
                }
                self.editor.set_status("Reverted to selected revision.");
                self.refresh_notes_after_change();
            }
        }

        {
            let mut lock = self.board_load_promise.lock();
            if let Some(board) = lock.take() {
                self.canvas.load_board(&board);
                self.current_board = Some(board);
                self.app_view = AppView::Canvas;
            }
        }

        {
            let mut lock = self.board_save_promise.lock();
            if let Some(board) = lock.take() {
                self.current_board = Some(board.clone());
                self.canvas.mark_saved();
                self.canvas.status_message = Some(("Board saved.".to_string(), 3.0));
            } else if let api::Promise::Failed(_) = &*lock {
                let failed = lock.take();
                if failed.is_some() {
                    self.canvas.save_in_flight = false;
                    self.canvas.status_message =
                        Some(("Board save failed.".to_string(), 3.0));
                }
            }
        }

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

        {
            let mut lock = self.boards_promise.lock();
            if let Some(boards) = lock.take() {
                self.boards = boards;
            }
        }

        {
            let mut lock = self.graph_promise.lock();
            if let Some(resp) = lock.take() {
                self.graph.set_graph(resp.nodes, resp.edges);
                self.graph_loaded = true;
                self.graph.set_status("Graph loaded.");
            } else if let api::Promise::Failed(msg) = std::mem::replace(&mut *lock, api::Promise::Idle) {
                self.graph.set_status(format!("Graph load failed: {msg}"));
            }
        }
    }

    fn refresh_notes_after_change(&mut self) {
        let mut lock = self.notes_promise.lock();
        match *lock {
            api::Promise::Ready(_) | api::Promise::Failed(_) | api::Promise::Idle => {
                *lock = api::Promise::Idle;
            }
            api::Promise::Pending => {}
        }
    }
}

impl eframe::App for WorkspaceApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        let dt = ctx.input(|i| i.stable_dt);

        if !self.bootstrapped {
            self.bootstrapped = true;
            self.refresh_notes(ctx);
            api::fetch_boards(self.boards_promise.clone(), ctx);
        }

        self.poll_promises();

        self.editor.tick_status(dt);
        self.canvas.tick_status(dt);
        self.graph.tick_status(dt);

        {
            let lock = self.notes_promise.lock();
            if matches!(*lock, api::Promise::Idle) {
                drop(lock);
                self.refresh_notes(ctx);
            }
        }

        #[cfg(feature = "egui")]
        {
            let style = crate::theme::agentic_style();
            ctx.set_style(style);
        }

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

        match self.app_view {
            AppView::Canvas => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let canvas_out = canvas::show(ui, &mut self.canvas);
                    self.handle_canvas_output(canvas_out, ctx);
                });
            }
            AppView::Graph => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let graph_out = graph_view::show(ui, &mut self.graph);
                    if graph_out.refresh_requested {
                        self.load_graph(ctx);
                    }
                });
            }
            AppView::Editor => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.add_space(8.0);
                    let editor_out = editor::show(ui, &mut self.editor);
                    self.handle_editor_output(editor_out, ctx);
                    ui.add_space(8.0);
                });
            }
        }
    }
}

impl WorkspaceApp {
    fn handle_sidebar_output(&mut self, out: SidebarOutput, ctx: &Context) {
        if out.open_graph {
            self.app_view = AppView::Graph;
            if !self.graph_loaded {
                self.load_graph(ctx);
            }
        }

        if let Some(note_id) = out.selected_note_id {
            self.load_note(&note_id, ctx);
            self.app_view = AppView::Editor;
        }
        if out.deselected {
            self.editor.unload();
        }
        if let Some(title) = out.create_note_title {
            self.pending_canvas_note = false;
            api::create_note(&title, "", Vec::new(), self.create_promise.clone(), ctx);
        }

        if let Some(board_id) = out.selected_board_id {
            self.load_board(&board_id, ctx);
        }
        if out.board_deselected {
            self.app_view = AppView::Editor;
            self.canvas = CanvasState::new();
            self.current_board = None;
        }
        if let Some(title) = out.create_board_title {
            api::create_board("default", &title, self.board_create_promise.clone(), ctx);
        }
    }

    fn handle_canvas_output(&mut self, out: CanvasOutput, ctx: &Context) {
        if let Some((board_id, doc)) = out.save_board {
            let workspace_id = self
                .current_board
                .as_ref()
                .map(|b| b.workspace_id.as_str())
                .unwrap_or("default");
            let project_id = self
                .current_board
                .as_ref()
                .and_then(|b| b.project_id.as_deref());
            let artifacts = derive_workspace_artifacts_from_canvas_document(
                &doc,
                &board_id,
                workspace_id,
                project_id,
            );
            let tldraw_value = serde_json::to_value(&doc).unwrap_or(serde_json::Value::Null);
            let title = self
                .current_board
                .as_ref()
                .map(|b| b.title.clone())
                .unwrap_or_default();
            api::save_board(
                &board_id,
                &title,
                &tldraw_value,
                artifacts.objects,
                artifacts.connectors,
                self.board_save_promise.clone(),
                ctx,
            );
            if let Some(ref mut board) = self.current_board {
                board.tldraw_document = tldraw_value;
            }
        }

        if let Some(world_pos) = out.create_note_at {
            self.pending_note_world_pos = Some(world_pos);
            self.pending_canvas_note = true;
            let note_number = self.notes.len() + 1;
            let title = format!("Canvas note {note_number}");
            let body = "# Canvas note\n\nCreated from the workspace canvas.";
            api::create_note(
                &title,
                body,
                vec!["workspace".to_string(), "canvas".to_string()],
                self.create_promise.clone(),
                ctx,
            );
        }

        if let Some(note_id) = out.open_note_id {
            self.load_note(&note_id, ctx);
            self.app_view = AppView::Editor;
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
