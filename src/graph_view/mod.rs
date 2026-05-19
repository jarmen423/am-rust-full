//! Interactive graph explorer — simple egui node/edge renderer.

use crate::model::{WorkspaceGraphEdge, WorkspaceGraphNode};
use crate::theme::color32;
use crate::theme::palette;
use egui::{Pos2, Rect, Stroke, Ui, Vec2};
use std::collections::HashMap;

/// Mutable state for the graph explorer view.
#[derive(Debug, Default)]
pub struct GraphViewState {
    pub nodes: Vec<WorkspaceGraphNode>,
    pub edges: Vec<WorkspaceGraphEdge>,
    pub selected_node_id: Option<String>,
    pub layout_positions: HashMap<String, Pos2>,
    pub status_message: Option<(String, f32)>,
}

impl GraphViewState {
    pub fn set_graph(&mut self, nodes: Vec<WorkspaceGraphNode>, edges: Vec<WorkspaceGraphEdge>) {
        self.nodes = nodes;
        self.edges = edges;
        self.selected_node_id = None;
        self.layout_positions.clear();
        self.layout_positions = compute_grid_layout(&self.nodes, 140.0);
    }

    pub fn tick_status(&mut self, dt: f32) {
        if let Some((_, ref mut remaining)) = self.status_message {
            *remaining -= dt;
            if *remaining <= 0.0 {
                self.status_message = None;
            }
        }
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some((msg.into(), 3.0));
    }
}

#[derive(Debug, Default)]
pub struct GraphViewOutput {
    pub refresh_requested: bool,
}

/// Render the graph view and return user actions.
pub fn show(ui: &mut Ui, state: &mut GraphViewState) -> GraphViewOutput {
    let mut output = GraphViewOutput::default();

    ui.horizontal(|ui| {
        ui.heading("Graph Explorer");
        if ui.button("Refresh").clicked() {
            output.refresh_requested = true;
        }
        if let Some((ref msg, _)) = state.status_message {
            ui.label(
                egui::RichText::new(msg)
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
        }
    });

    ui.add_space(4.0);
    ui.separator();

    if state.nodes.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() / 2.0 - 20.0);
            ui.label(
                egui::RichText::new("No graph nodes yet")
                    .color(ui.visuals().weak_text_color()),
            );
            ui.label(
                egui::RichText::new("Click Refresh to load from /api/workspace/graph/explore")
                    .small(),
            );
        });
        return output;
    }

    let graph_rect = ui.available_rect_before_wrap();
    let painter = ui.painter_at(graph_rect);

    // Edges
    for edge in &state.edges {
        let Some(from) = state.layout_positions.get(&edge.from_node_id) else {
            continue;
        };
        let Some(to) = state.layout_positions.get(&edge.to_node_id) else {
            continue;
        };
        painter.line_segment(
            [*from, *to],
            Stroke::new(1.5, color32(palette::ACCENT_SECONDARY)),
        );
    }

    // Nodes
    for node in &state.nodes {
        let Some(center) = state.layout_positions.get(&node.node_id) else {
            continue;
        };
        let radius = 22.0;
        let node_rect = Rect::from_center_size(*center, Vec2::splat(radius * 2.0));
        let selected = state.selected_node_id.as_deref() == Some(node.node_id.as_str());
        let fill = if selected {
            color32(palette::ACCENT_PRIMARY)
        } else {
            color32(palette::BG_ELEVATED)
        };
        let stroke = if selected {
            Stroke::new(2.0, color32(palette::ACCENT_PRIMARY))
        } else {
            Stroke::new(1.0, color32(palette::BORDER))
        };

        painter.circle_filled(*center, radius, fill);
        painter.circle_stroke(*center, radius, stroke);

        let label = truncate_label(&node.title, 14);
        painter.text(
            *center + Vec2::new(0.0, radius + 10.0),
            egui::Align2::CENTER_TOP,
            label,
            egui::FontId::proportional(11.0),
            color32(palette::TEXT_PRIMARY),
        );

        let response = ui.interact(node_rect, ui.id().with(&node.node_id), egui::Sense::click());
        if response.clicked() {
            state.selected_node_id = Some(node.node_id.clone());
        }
    }

    // Inspector panel
    ui.allocate_ui_at_rect(
        Rect::from_min_size(
            graph_rect.right_top() - Vec2::new(260.0, 0.0),
            Vec2::new(250.0, graph_rect.height()),
        ),
        |ui| {
            egui::Frame::none()
                .fill(color32(palette::BG_ELEVATED))
                .inner_margin(12.0)
                .rounding(6.0)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Inspector").strong());
                    ui.separator();
                    if let Some(ref id) = state.selected_node_id {
                        if let Some(node) = state.nodes.iter().find(|n| &n.node_id == id) {
                            ui.label(format!("Title: {}", node.title));
                            if let Some(ref subtitle) = node.subtitle {
                                ui.label(format!("Subtitle: {subtitle}"));
                            }
                            ui.label(format!("Type: {}", node.node_type));
                            ui.label(format!("ID: {}", node.node_id));
                        } else {
                            ui.label("Node not found.");
                        }
                    } else {
                        ui.label(
                            egui::RichText::new("Click a node to inspect")
                                .color(ui.visuals().weak_text_color()),
                        );
                    }
                });
        },
    );

    output
}

fn compute_grid_layout(nodes: &[WorkspaceGraphNode], spacing: f32) -> HashMap<String, Pos2> {
    let mut positions = HashMap::new();
    let cols = (nodes.len() as f32).sqrt().ceil().max(1.0) as usize;
    for (idx, node) in nodes.iter().enumerate() {
        let row = idx / cols;
        let col = idx % cols;
        positions.insert(
            node.node_id.clone(),
            Pos2::new(
                80.0 + col as f32 * spacing,
                80.0 + row as f32 * spacing,
            ),
        );
    }
    positions
}

fn truncate_label(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        format!("{}…", text.chars().take(max).collect::<String>())
    }
}
