//! Sidebar panel — note list, search, and navigation.

use crate::model::{WorkspaceBoard, WorkspaceNoteDocument};
use egui::{RichText, ScrollArea, TextEdit, Ui};

/// Mutable state for the sidebar.
#[derive(Debug, Default)]
pub struct SidebarState {
    /// Search / filter text.
    pub search: String,
    /// Whether to show the new-note title input.
    pub show_new_note_input: bool,
    /// Draft title for a new note.
    pub new_note_title: String,
    /// Currently selected note id.
    pub selected_note_id: Option<String>,
    /// Currently selected board id.
    pub selected_board_id: Option<String>,
    /// Whether to show the new-board title input.
    pub show_new_board_input: bool,
    /// Draft title for a new board.
    pub new_board_title: String,
}

impl SidebarState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Select a note by id.
    pub fn select_note(&mut self, note_id: &str) {
        self.selected_note_id = Some(note_id.to_string());
    }

    /// Clear selection.
    pub fn deselect(&mut self) {
        self.selected_note_id = None;
    }

    /// Start creating a new note.
    pub fn start_new_note(&mut self) {
        self.show_new_note_input = true;
        self.new_note_title.clear();
        self.deselect();
    }

    /// Cancel new-note creation.
    pub fn cancel_new_note(&mut self) {
        self.show_new_note_input = false;
        self.new_note_title.clear();
    }

    /// Select a board by id.
    pub fn select_board(&mut self, board_id: &str) {
        self.selected_board_id = Some(board_id.to_string());
    }

    /// Deselect current board.
    pub fn deselect_board(&mut self) {
        self.selected_board_id = None;
    }

    /// Start creating a new board.
    pub fn start_new_board(&mut self) {
        self.show_new_board_input = true;
        self.new_board_title.clear();
    }

    /// Cancel new-board creation.
    pub fn cancel_new_board(&mut self) {
        self.show_new_board_input = false;
        self.new_board_title.clear();
    }
}

/// Show the sidebar UI.
///
/// Returns `Some(note_id)` if the user clicked a different note.
pub fn show(
    ui: &mut Ui,
    state: &mut SidebarState,
    notes: &[WorkspaceNoteDocument],
    boards: &[WorkspaceBoard],
) -> SidebarOutput {
    let mut output = SidebarOutput::default();

    ui.horizontal(|ui| {
        ui.heading(RichText::new("Notes").size(16.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("+ New").clicked() {
                state.start_new_note();
            }
        });
    });

    ui.add_space(4.0);

    // ── Search ──────────────────────────────────────────────────────
    ui.add(
        TextEdit::singleline(&mut state.search)
            .hint_text("Search notes...")
            .margin(egui::vec2(6.0, 4.0)),
    );

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);

    // ── New note title input ────────────────────────────────────────
    if state.show_new_note_input {
        ui.group(|ui| {
            ui.label("New note title:");
            let response = ui.add(
                TextEdit::singleline(&mut state.new_note_title)
                    .hint_text("Untitled Note")
                    .desired_width(f32::INFINITY),
            );
            // Enter key confirms
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let title = if state.new_note_title.trim().is_empty() {
                    "Untitled Note"
                } else {
                    &state.new_note_title
                };
                output.create_note_title = Some(title.to_string());
                state.cancel_new_note();
            }
            ui.horizontal(|ui| {
                if ui.button("Create").clicked() {
                    let title = if state.new_note_title.trim().is_empty() {
                        "Untitled Note"
                    } else {
                        &state.new_note_title
                    };
                    output.create_note_title = Some(title.to_string());
                    state.cancel_new_note();
                }
                if ui.button("Cancel").clicked() {
                    state.cancel_new_note();
                }
            });
        });
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);
    }

    // ── Note list ───────────────────────────────────────────────────
    let filter = state.search.to_lowercase();
    let filtered: Vec<&WorkspaceNoteDocument> = notes
        .iter()
        .filter(|n| {
            filter.is_empty()
                || n.title.to_lowercase().contains(&filter)
                || n.tags.iter().any(|t| t.to_lowercase().contains(&filter))
                || n.body_markdown.to_lowercase().contains(&filter)
        })
        .collect();

    if filtered.is_empty() && !notes.is_empty() {
        ui.label(RichText::new("No matching notes").color(ui.visuals().weak_text_color()));
    } else if notes.is_empty() {
        ui.label(RichText::new("No notes yet").color(ui.visuals().weak_text_color()));
        ui.label(RichText::new("Click '+ New' to create one.").small());
    }

    ScrollArea::vertical().id_salt("note_list").show_rows(
        ui,
        ui.text_style_height(&egui::TextStyle::Body),
        filtered.len(),
        |ui, row_range| {
            for note in filtered.iter().skip(row_range.start).take(row_range.len()) {
                let is_selected = state
                    .selected_note_id
                    .as_ref()
                    .map(|id| id == &note.note_id)
                    .unwrap_or(false);

                let bg = if is_selected {
                    ui.visuals().selection.bg_fill
                } else {
                    ui.visuals().extreme_bg_color
                };

                let response = egui::Frame::none()
                    .fill(bg)
                    .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                    .rounding(4.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(&note.title)
                                        .strong()
                                        .size(13.0)
                                        .color(if is_selected {
                                            ui.visuals().selection.stroke.color
                                        } else {
                                            ui.visuals().text_color()
                                        }),
                                );
                                if !note.tags.is_empty() {
                                    ui.horizontal_wrapped(|ui| {
                                        for tag in &note.tags {
                                            ui.add(
                                                egui::Label::new(
                                                    RichText::new(format!(" #{tag}"))
                                                        .small()
                                                        .color(ui.visuals().weak_text_color()),
                                                )
                                                .selectable(false),
                                            );
                                        }
                                    });
                                }
                            });
                        });
                    })
                    .response
                    .interact(egui::Sense::click());

                if response.clicked() && !is_selected {
                    state.select_note(&note.note_id);
                    output.selected_note_id = Some(note.note_id.clone());
                }

                if response.clicked() && is_selected {
                    // Click again to deselect
                    state.deselect();
                    output.deselected = true;
                }

                ui.add_space(2.0);
            }
        },
    );

    // ── Board section ───────────────────────────────────────────────
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.heading(RichText::new("Boards").size(16.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("+ New").clicked() {
                state.start_new_board();
            }
        });
    });

    ui.add_space(4.0);

    // New board title input
    if state.show_new_board_input {
        ui.group(|ui| {
            ui.label("New board title:");
            let response = ui.add(
                TextEdit::singleline(&mut state.new_board_title)
                    .hint_text("Untitled Board")
                    .desired_width(f32::INFINITY),
            );
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let title = if state.new_board_title.trim().is_empty() {
                    "Untitled Board"
                } else {
                    &state.new_board_title
                };
                output.create_board_title = Some(title.to_string());
                state.cancel_new_board();
            }
            ui.horizontal(|ui| {
                if ui.button("Create").clicked() {
                    let title = if state.new_board_title.trim().is_empty() {
                        "Untitled Board"
                    } else {
                        &state.new_board_title
                    };
                    output.create_board_title = Some(title.to_string());
                    state.cancel_new_board();
                }
                if ui.button("Cancel").clicked() {
                    state.cancel_new_board();
                }
            });
        });
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);
    }

    // Board list
    if boards.is_empty() {
        ui.label(RichText::new("No boards yet").color(ui.visuals().weak_text_color()));
        ui.label(RichText::new("Click '+ New' to create one.").small());
    } else {
        for board in boards {
            let is_selected = state
                .selected_board_id
                .as_ref()
                .map(|id| id == &board.board_id)
                .unwrap_or(false);

            let bg = if is_selected {
                ui.visuals().selection.bg_fill
            } else {
                ui.visuals().extreme_bg_color
            };

            let response = egui::Frame::none()
                .fill(bg)
                .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                .rounding(4.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(&board.title)
                                    .strong()
                                    .size(13.0)
                                    .color(if is_selected {
                                        ui.visuals().selection.stroke.color
                                    } else {
                                        ui.visuals().text_color()
                                    }),
                            );
                            if !board.tags.is_empty() {
                                ui.horizontal_wrapped(|ui| {
                                    for tag in &board.tags {
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(format!(" #{tag}"))
                                                    .small()
                                                    .color(ui.visuals().weak_text_color()),
                                            )
                                            .selectable(false),
                                        );
                                    }
                                });
                            }
                        });
                    });
                })
                .response
                .interact(egui::Sense::click());

            if response.clicked() && !is_selected {
                state.select_board(&board.board_id);
                output.selected_board_id = Some(board.board_id.clone());
                // Deselect note when selecting a board
                state.deselect();
            }

            if response.clicked() && is_selected {
                state.deselect_board();
                output.board_deselected = true;
            }

            ui.add_space(2.0);
        }
    }

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.heading(RichText::new("Views").size(16.0));
    });
    ui.add_space(4.0);
    if ui.button("Graph Explorer").clicked() {
        output.open_graph = true;
    }

    output
}

/// Actions produced by the sidebar UI.
#[derive(Debug, Default)]
pub struct SidebarOutput {
    /// User clicked a note — load it into editor.
    pub selected_note_id: Option<String>,
    /// User deselected the current note.
    pub deselected: bool,
    /// User wants to create a note with this title.
    pub create_note_title: Option<String>,
    /// User clicked a board — show it in canvas.
    pub selected_board_id: Option<String>,
    /// User deselected the current board.
    pub board_deselected: bool,
    /// User wants to create a board with this title.
    pub create_board_title: Option<String>,
    /// User opened the graph explorer view.
    pub open_graph: bool,
}
