//! Note editor panel — title, tags, markdown body, save, history.

use crate::model::{NoteHistoryItem, WorkspaceNoteDocument};
use egui::{RichText, ScrollArea, TextEdit, Ui};

/// Mutable state for the note editor.
#[derive(Debug, Default)]
pub struct EditorState {
    /// Note being edited (cached from last load).
    pub note: Option<WorkspaceNoteDocument>,
    /// Dirty flag — true if user has modified the draft.
    pub dirty: bool,
    /// Draft title.
    pub draft_title: String,
    /// Draft tags (comma-separated).
    pub draft_tags: String,
    /// Draft body (markdown).
    pub draft_body: String,
    /// Show the history panel.
    pub show_history: bool,
    /// History items (fetched from API).
    pub history: Vec<NoteHistoryItem>,
    /// Status message to show the user.
    pub status_message: Option<(String, f32)>, // (message, time_remaining)
}

impl EditorState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load a note into the editor (resets dirty flag).
    pub fn load_note(&mut self, note: &WorkspaceNoteDocument) {
        self.note = Some(note.clone());
        self.draft_title = note.title.clone();
        self.draft_tags = note.tags.join(", ");
        self.draft_body = note.body_markdown.clone();
        self.dirty = false;
        self.show_history = false;
        self.history.clear();
    }

    /// Unload the current note (clears editor).
    pub fn unload(&mut self) {
        self.note = None;
        self.draft_title.clear();
        self.draft_tags.clear();
        self.draft_body.clear();
        self.dirty = false;
        self.show_history = false;
        self.history.clear();
    }

    /// Mark the editor as dirty if drafts diverge from loaded note.
    pub fn check_dirty(&mut self) {
        if let Some(ref note) = self.note {
            let current_tags: Vec<String> = self
                .draft_tags
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            self.dirty = self.draft_title != note.title
                || current_tags != note.tags
                || self.draft_body != note.body_markdown;
        }
    }

    /// Build tag Vec from the comma-separated draft.
    pub fn parse_tags(&self) -> Vec<String> {
        self.draft_tags
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Show a status message that fades after N seconds.
    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some((msg.into(), 3.0));
    }

    /// Tick the status message timer.
    pub fn tick_status(&mut self, dt: f32) {
        if let Some((_, ref mut remaining)) = self.status_message {
            *remaining -= dt;
            if *remaining <= 0.0 {
                self.status_message = None;
            }
        }
    }
}

/// Show the editor UI.
///
/// Returns `EditorOutput` describing what actions the user took.
pub fn show(ui: &mut Ui, state: &mut EditorState) -> EditorOutput {
    let mut output = EditorOutput::default();

    if state.note.is_none() {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() / 2.0 - 20.0);
            ui.label(
                RichText::new("Select a note from the sidebar, or create a new one.")
                    .color(ui.visuals().weak_text_color()),
            );
        });
        return output;
    }

    let note_id = state.note.as_ref().unwrap().note_id.clone();

    // ── Toolbar ─────────────────────────────────────────────────────
    ui.horizontal(|ui| {
        // Title
        let title_response = ui.add(
            TextEdit::singleline(&mut state.draft_title)
                .font(egui::TextStyle::Heading)
                .desired_width(300.0)
                .hint_text("Note title..."),
        );
        if title_response.changed() {
            state.check_dirty();
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // History toggle
            let history_label = if state.show_history {
                "Hide History"
            } else {
                "History"
            };
            if ui.button(history_label).clicked() {
                state.show_history = !state.show_history;
                if state.show_history {
                    output.fetch_history = Some(note_id.clone());
                }
            }

            // Save button
            let save_btn = ui.add_sized(
                [60.0, 24.0],
                egui::Button::new(RichText::new("Save").strong())
                    .fill(if state.dirty {
                        ui.visuals().selection.bg_fill
                    } else {
                        ui.visuals().widgets.inactive.bg_fill
                    }),
            );
            if save_btn.clicked() && state.dirty {
                output.save_note = Some(SaveRequest {
                    note_id: note_id.clone(),
                    title: state.draft_title.clone(),
                    body_markdown: state.draft_body.clone(),
                    tags: state.parse_tags(),
                });
            }
        });
    });

    ui.add_space(4.0);

    // ── Tags ────────────────────────────────────────────────────────
    ui.horizontal(|ui| {
        ui.label(RichText::new("Tags:").small());
        let tag_response = ui.add(
            TextEdit::singleline(&mut state.draft_tags)
                .hint_text("tag1, tag2, tag3")
                .desired_width(f32::INFINITY),
        );
        if tag_response.changed() {
            state.check_dirty();
        }
    });

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);

    // ── Body (markdown) ─────────────────────────────────────────────
    let available = ui.available_rect_before_wrap();
    let body_height = if state.show_history {
        available.height() * 0.55
    } else {
        available.height() - 30.0 // leave room for status
    };

    ScrollArea::vertical()
        .id_salt("editor_body")
        .max_height(body_height)
        .show(ui, |ui| {
            let body_response = ui.add(
                TextEdit::multiline(&mut state.draft_body)
                    .code_editor()
                    .desired_width(f32::INFINITY)
                    .desired_rows(20)
                    .hint_text("Write markdown here..."),
            );
            if body_response.changed() {
                state.check_dirty();
            }
        });

    // ── Status message ──────────────────────────────────────────────
    if let Some((ref msg, _)) = state.status_message {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(msg).small().color(ui.visuals().warn_fg_color));
        });
    }

    // ── History panel ───────────────────────────────────────────────
    if state.show_history {
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        ui.heading(RichText::new("Git History").size(14.0));
        ui.add_space(4.0);

        if state.history.is_empty() {
            ui.label(RichText::new("No history available.").small());
        } else {
            ScrollArea::vertical()
                .id_salt("history_panel")
                .max_height(available.height() * 0.35)
                .show(ui, |ui| {
                    for item in &state.history {
                        ui.horizontal(|ui| {
                            // Short SHA
                            let short_sha = if item.sha.len() > 7 {
                                &item.sha[..7]
                            } else {
                                &item.sha
                            };
                            ui.monospace(RichText::new(short_sha).small());
                            ui.separator();
                            // Timestamp
                            ui.label(RichText::new(&item.timestamp).small());
                            ui.separator();
                            // Subject
                            ui.label(RichText::new(&item.subject).small().strong());
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button("Revert").clicked() {
                                        output.revert_request = Some((
                                            note_id.clone(),
                                            item.sha.clone(),
                                        ));
                                    }
                                },
                            );
                        });
                        ui.add_space(2.0);
                    }
                });
        }
    }

    output
}

/// A request to save a note.
#[derive(Debug, Clone)]
pub struct SaveRequest {
    pub note_id: String,
    pub title: String,
    pub body_markdown: String,
    pub tags: Vec<String>,
}

/// Actions produced by the editor UI.
#[derive(Debug, Default)]
pub struct EditorOutput {
    /// User clicked Save — contains the save request.
    pub save_note: Option<SaveRequest>,
    /// User clicked History — fetch history for this note.
    pub fetch_history: Option<String>,
    /// User clicked Revert — (note_id, revision_sha).
    pub revert_request: Option<(String, String)>,
}
