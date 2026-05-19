//! Card rendering — note cards, text cards, and graph-reference cards on the canvas.

use crate::canvas::camera::Camera;
use crate::model::{CanvasObject, NOTE_CARD_HEIGHT, NOTE_CARD_WIDTH};
use crate::theme::color32;
use crate::theme::palette;
use egui::{Pos2, Rect, Rounding, Stroke, Ui, Vec2};

/// Which edge of a card the pointer is near.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

/// Interaction state returned after rendering a card.
#[derive(Debug, Default)]
pub struct CardInteraction {
    /// True if the card was clicked this frame.
    pub clicked: bool,
    /// True if the card was double-clicked this frame.
    pub double_clicked: bool,
    /// If the card is being dragged, the delta in **world** space.
    pub dragged: Option<Vec2>,
    /// Which edge the pointer is hovering over.
    pub hover_edge: Option<Edge>,
    /// Current pointer position in screen space.
    pub pointer_pos: Option<Pos2>,
}

/// Render a single card on the canvas.
///
/// The card is drawn as a rounded rectangle at the object's world position
/// (converted to screen space via `camera`).  Returns interaction info so
/// the caller can handle selection, dragging, and edge-hovers.
pub fn render_card(
    ui: &mut Ui,
    obj: &CanvasObject,
    camera: &Camera,
    is_selected: bool,
    is_hovered: bool,
) -> CardInteraction {
    let mut interaction = CardInteraction::default();

    // ── Size ──────────────────────────────────────────────────────────
    let w = if obj.w > 0.0 { obj.w } else { NOTE_CARD_WIDTH };
    let h = if obj.h > 0.0 { obj.h } else { NOTE_CARD_HEIGHT };

    // ── Position (world → screen) ────────────────────────────────────
    let screen_top_left = camera.world_to_screen(Pos2::new(obj.x, obj.y), &ui.max_rect());
    let rect = Rect::from_min_size(screen_top_left, Vec2::new(w * camera.zoom, h * camera.zoom));

    // ── Pointer state ─────────────────────────────────────────────────
    let pointer = ui.input(|i| i.pointer.hover_pos());
    interaction.pointer_pos = pointer;

    let pointer_in_rect = pointer.map_or(false, |p| rect.contains(p));
    if pointer_in_rect {
        interaction.hover_edge = detect_edge(pointer.unwrap(), rect);
    }

    // ── Background fill ───────────────────────────────────────────────
    let fill_color = color32(palette::BG_ELEVATED);
    let stroke_color = if is_selected {
        color32(palette::ACCENT_PRIMARY)
    } else {
        color32(palette::BORDER)
    };
    let stroke_width = if is_selected { 2.0 } else { 1.0 };

    ui.painter().rect_filled(rect, Rounding::same(8.0 * camera.zoom), fill_color);
    ui.painter().rect_stroke(
        rect,
        Rounding::same(8.0 * camera.zoom),
        Stroke::new(stroke_width, stroke_color),
    );

    // ── Inner padding (scaled by zoom) ────────────────────────────────
    let pad = 12.0 * camera.zoom;
    let min_inner = rect.min + Vec2::splat(pad);
    let max_inner = rect.max - Vec2::splat(pad);
    let inner_rect = Rect::from_min_max(min_inner, max_inner);

    // ── Title ─────────────────────────────────────────────────────────
    if !obj.title.is_empty() && inner_rect.min.x < inner_rect.max.x {
        let title_size = (14.0 * camera.zoom).max(8.0);
        let galley = ui.painter().layout(
            obj.title.clone(),
            egui::FontId::proportional(title_size),
            color32(palette::TEXT_PRIMARY),
            inner_rect.width(),
        );
        let title_pos = inner_rect.left_top();
        // Only draw if we have vertical room
        if title_pos.y + galley.rect.height() <= rect.max.y - pad {
            ui.painter().galley(title_pos, galley, egui::Color32::WHITE);
        }
    }

    // ── Markdown preview (2 lines max) ────────────────────────────────
    let title_height = (18.0 * camera.zoom).max(10.0);
    let preview_y = inner_rect.min.y + title_height;
    if !obj.markdown_preview.is_empty()
        && preview_y < rect.max.y - pad
        && inner_rect.min.x < inner_rect.max.x
    {
        let preview = truncate_two_lines(&obj.markdown_preview);
        let preview_size = (12.0 * camera.zoom).max(7.0);
        let galley = ui.painter().layout(
            preview,
            egui::FontId::proportional(preview_size),
            color32(palette::TEXT_SECONDARY),
            inner_rect.width(),
        );
        let preview_pos = Pos2::new(inner_rect.min.x, preview_y);
        if preview_pos.y + galley.rect.height() <= rect.max.y - pad {
            ui.painter().galley(preview_pos, galley, egui::Color32::WHITE);
        }
    }

    // ── Tags (small pills) ────────────────────────────────────────────
    let tags_y = preview_y + (36.0 * camera.zoom).min(h * camera.zoom * 0.4);
    if !obj.tags.is_empty() && tags_y < rect.max.y - pad && inner_rect.min.x < inner_rect.max.x {
        let tag_size = (10.0 * camera.zoom).max(6.0);
        let tag_font = egui::FontId::proportional(tag_size);
        let mut tag_x = inner_rect.min.x;
        let tag_row_height = tag_size + 4.0 * camera.zoom;

        for tag in &obj.tags {
            let galley = ui
                .painter()
                .layout(tag.clone(), tag_font.clone(), color32(palette::ACCENT_PRIMARY), f32::INFINITY);
            let tag_width = galley.rect.width() + 8.0 * camera.zoom;

            // Wrap to next row if needed
            if tag_x + tag_width > inner_rect.max.x && tag_x > inner_rect.min.x {
                tag_x = inner_rect.min.x;
            }

            let tag_rect = Rect::from_min_size(
                Pos2::new(tag_x, tags_y),
                Vec2::new(tag_width, tag_row_height),
            );

            // Only draw if there's room
            if tag_rect.max.y <= rect.max.y - pad && tag_rect.max.x <= inner_rect.max.x {
                // Darker background for tag pill
                let tag_bg = color32(palette::BG_DARK);
                ui.painter().rect_filled(
                    tag_rect,
                    Rounding::same(4.0 * camera.zoom),
                    tag_bg,
                );
                ui.painter().galley(
                    tag_rect.left_center() - Vec2::new(0.0, galley.rect.height() / 2.0),
                    galley,
                    egui::Color32::WHITE,
                );
            }

            tag_x += tag_width + 4.0 * camera.zoom;
        }
    }

    // ── Connection dots (visible on hover) ────────────────────────────
    if is_hovered || pointer_in_rect {
        let dot_radius = 4.0 * camera.zoom;
        let dot_color = color32(palette::ACCENT_SECONDARY);
        let mid_top = Pos2::new(rect.center().x, rect.min.y);
        let mid_bottom = Pos2::new(rect.center().x, rect.max.y);
        let mid_left = Pos2::new(rect.min.x, rect.center().y);
        let mid_right = Pos2::new(rect.max.x, rect.center().y);

        for pos in [mid_top, mid_bottom, mid_left, mid_right] {
            ui.painter().circle_filled(pos, dot_radius, dot_color);
        }
    }

    // ── Interaction handling ──────────────────────────────────────────
    // We use an invisible button covering the card rect to capture pointer events.
    let response = ui.interact(rect, ui.id().with(&obj.id), egui::Sense::click_and_drag());

    interaction.clicked = response.clicked();
    interaction.double_clicked = response.double_clicked();
    if response.dragged() {
        interaction.dragged = Some(response.drag_delta() / camera.zoom);
    }

    interaction
}

/// Detect which edge of `rect` the pointer is closest to.
///
/// Returns `None` if the pointer is well inside the rect (more than
/// 10 px / zoom from any edge).
fn detect_edge(pointer: Pos2, rect: Rect) -> Option<Edge> {
    let threshold = 10.0;
    let dist_top = (pointer.y - rect.min.y).abs();
    let dist_bottom = (pointer.y - rect.max.y).abs();
    let dist_left = (pointer.x - rect.min.x).abs();
    let dist_right = (pointer.x - rect.max.x).abs();

    let min_dist = dist_top.min(dist_bottom).min(dist_left).min(dist_right);
    if min_dist > threshold {
        return None;
    }

    if min_dist == dist_top {
        Some(Edge::Top)
    } else if min_dist == dist_bottom {
        Some(Edge::Bottom)
    } else if min_dist == dist_left {
        Some(Edge::Left)
    } else {
        Some(Edge::Right)
    }
}

/// Truncate text to roughly two lines by taking the first newline or
/// clamping at ~80 characters, whichever comes first.
fn truncate_two_lines(text: &str) -> String {
    let mut lines = 0;
    let mut result = String::new();
    for ch in text.chars() {
        if ch == '\n' {
            lines += 1;
            if lines >= 2 {
                result.push_str("…");
                break;
            }
            result.push(' '); // collapse newlines to spaces
        } else {
            result.push(ch);
        }
    }
    // Hard cap at 120 chars to prevent overflow on very long single lines
    if result.len() > 120 {
        result.truncate(120);
        result.push('…');
    }
    result
}
