//! Read-only Cypher operator shell.

use crate::api::{Promise, SharedPromise};
use egui::{ScrollArea, TextEdit, Ui};

#[derive(Debug, Default)]
pub struct QueryShellState {
    pub cypher: String,
    pub last_columns: Vec<String>,
    pub last_rows: Vec<Vec<String>>,
    pub last_error: Option<String>,
    pub last_attempt_id: Option<String>,
    pub status_message: Option<String>,
}

pub struct QueryShellOutput {
    pub execute: bool,
}

impl Default for QueryShellOutput {
    fn default() -> Self {
        Self {
            execute: false,
        }
    }
}

pub fn show(ui: &mut Ui, state: &mut QueryShellState) -> QueryShellOutput {
    let mut out = QueryShellOutput::default();
    ui.heading("Cypher shell (read-only)");
    ui.label(
        "Bounded MATCH/RETURN only. Mutations are rejected server-side.",
    );
    ui.add_space(4.0);
    ui.add(
        TextEdit::multiline(&mut state.cypher)
            .code_editor()
            .desired_rows(6)
            .hint_text("MATCH (n) RETURN n LIMIT 10"),
    );
    if ui.button("Run query").clicked() {
        out.execute = true;
    }
    if let Some(ref err) = state.last_error {
        ui.colored_label(ui.visuals().error_fg_color, err);
    }
    if let Some(ref id) = state.last_attempt_id {
        ui.label(format!("attempt_id: {id}"));
    }
    if !state.last_rows.is_empty() || !state.last_columns.is_empty() {
        ui.separator();
        ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
            for row in &state.last_rows {
                ui.monospace(row.join(" | "));
            }
        });
    }
    if let Some(ref msg) = state.status_message {
        ui.label(msg);
    }
    out
}

#[derive(Debug, Clone)]
pub struct QueryResultView {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub attempt_id: String,
}

pub fn poll_execute(
    promise: &SharedPromise<Result<QueryResultView, String>>,
    state: &mut QueryShellState,
) {
    let mut lock = promise.lock();
    match lock.take() {
        Some(Ok(view)) => {
            state.last_columns = view.columns;
            state.last_rows = view.rows;
            state.last_attempt_id = Some(view.attempt_id);
            state.last_error = None;
            state.status_message = Some(format!("{} rows", state.last_rows.len()));
        }
        Some(Err(e)) => {
            state.last_error = Some(e.clone());
            state.status_message = None;
        }
        None => {}
    }
    if let Promise::Failed(e) = std::mem::replace(&mut *lock, Promise::Idle) {
        state.last_error = Some(e);
    }
}
