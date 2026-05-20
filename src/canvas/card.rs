//! Card rendering — note cards, text cards, and graph-reference cards on the canvas.

use crate::canvas::camera::Camera;
use crate::canvas::markdown_preview::paint_markdown_preview;
use crate::canvas::tools::CanvasTool;
use crate::model::{CanvasObject, NOTE_CARD_HEIGHT, NOTE_CARD_WIDTH};
use crate::theme::color32;
use crate::theme::palette;
use egui::{CursorIcon, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2};

/// Which edge of a card the pointer is near.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

/// Corner resize handle (maps to two edges).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    Nw,
    Ne,
    Sw,
    Se,
}

/// Border resize target — edge or corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeHandle {
    Edge(Edge),
    Corner(Corner),
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
    /// Active border resize drag (world-space delta per frame).
    pub resize_drag: Option<(ResizeHandle, Vec2)>,
    /// Border zone under the pointer (for selection / cursor).
    pub hover_resize: Option<ResizeHandle>,
    /// Current pointer position in screen space.
    pub pointer_pos: Option<Pos2>,
}

/// Render a single card on the canvas.
///
/// The card is drawn as a rounded rectangle at the object's world position
/// (converted to screen space via `camera`).  Returns interaction info so
/// the caller can handle selection, dragging, and border resize.
pub fn render_card(
    ui: &mut Ui,
    obj: &CanvasObject,
    camera: &Camera,
    viewport: Rect,
    tool: CanvasTool,
    is_selected: bool,
    is_hovered: bool,
) -> CardInteraction {
    let mut interaction = CardInteraction::default();

    // ── Size ──────────────────────────────────────────────────────────
    let w = if obj.w > 0.0 { obj.w } else { NOTE_CARD_WIDTH };
    let h = if obj.h > 0.0 { obj.h } else { NOTE_CARD_HEIGHT };

    // ── Position (world → screen) ────────────────────────────────────
    let screen_top_left = camera.world_to_screen(Pos2::new(obj.x, obj.y), &viewport);
    let rect = Rect::from_min_size(screen_top_left, Vec2::new(w * camera.zoom, h * camera.zoom));

    // ── Pointer state ─────────────────────────────────────────────────
    let pointer = ui.input(|i| i.pointer.hover_pos());
    interaction.pointer_pos = pointer;

    let pointer_in_rect = pointer.map_or(false, |p| rect.contains(p));
    let margin = resize_margin(camera.zoom);
    let border_resize_active =
        tool == CanvasTool::Select && (is_selected || pointer_in_rect);

    if border_resize_active {
        if let Some(p) = pointer {
            interaction.hover_resize = classify_resize_zone(p, rect, camera.zoom);
            if let Some(handle) = interaction.hover_resize {
                ui.ctx().set_cursor_icon(resize_cursor(handle));
            }
        }
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
        if title_pos.y + galley.rect.height() <= rect.max.y - pad {
            ui.painter().galley(title_pos, galley, egui::Color32::WHITE);
        }
    }

    // ── Markdown preview (headings, bold, line breaks) ─────────────────
    let title_height = (18.0 * camera.zoom).max(10.0);
    let preview_y = inner_rect.min.y + title_height;
    let tag_row_reserve = (22.0 * camera.zoom).max(14.0);
    let preview_bottom = if !obj.markdown_preview.is_empty()
        && preview_y < rect.max.y - pad - tag_row_reserve
        && inner_rect.min.x < inner_rect.max.x
    {
        paint_markdown_preview(
            ui.painter(),
            &obj.markdown_preview,
            Pos2::new(inner_rect.min.x, preview_y),
            inner_rect.width(),
            rect.max.y - pad - tag_row_reserve,
            camera.zoom,
        )
    } else {
        preview_y
    };

    // ── Tags (small pills) ────────────────────────────────────────────
    let tags_y = preview_bottom + (6.0 * camera.zoom);
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

            if tag_x + tag_width > inner_rect.max.x && tag_x > inner_rect.min.x {
                tag_x = inner_rect.min.x;
            }

            let tag_rect = Rect::from_min_size(
                Pos2::new(tag_x, tags_y),
                Vec2::new(tag_width, tag_row_height),
            );

            if tag_rect.max.y <= rect.max.y - pad && tag_rect.max.x <= inner_rect.max.x {
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

    // ── Connection dots (Connect tool only) ───────────────────────────
    if tool == CanvasTool::Connect && (is_hovered || pointer_in_rect) {
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
    let interior_rect = if border_resize_active {
        rect.shrink(margin)
    } else {
        rect
    };

    let body_response = ui.interact(
        interior_rect,
        ui.id().with(&obj.id),
        Sense::click_and_drag(),
    );

    interaction.clicked = body_response.clicked();
    interaction.double_clicked = body_response.double_clicked();
    if body_response.dragged() {
        interaction.dragged = Some(body_response.drag_delta() / camera.zoom);
    }

    if border_resize_active {
        register_border_resize_interacts(
            ui,
            obj,
            rect,
            margin,
            camera.zoom,
            &mut interaction,
        );
    }

    interaction
}

/// Invisible edge strips and corner caps (registered after body; corners win at vertices).
fn register_border_resize_interacts(
    ui: &mut Ui,
    obj: &CanvasObject,
    rect: Rect,
    margin: f32,
    zoom: f32,
    interaction: &mut CardInteraction,
) {
    let cap = margin;

    let edge_strips = [
        (
            Edge::Top,
            Rect::from_min_max(
                Pos2::new(rect.min.x + cap, rect.min.y),
                Pos2::new(rect.max.x - cap, rect.min.y + margin),
            ),
        ),
        (
            Edge::Bottom,
            Rect::from_min_max(
                Pos2::new(rect.min.x + cap, rect.max.y - margin),
                Pos2::new(rect.max.x - cap, rect.max.y),
            ),
        ),
        (
            Edge::Left,
            Rect::from_min_max(
                Pos2::new(rect.min.x, rect.min.y + cap),
                Pos2::new(rect.min.x + margin, rect.max.y - cap),
            ),
        ),
        (
            Edge::Right,
            Rect::from_min_max(
                Pos2::new(rect.max.x - margin, rect.min.y + cap),
                Pos2::new(rect.max.x, rect.max.y - cap),
            ),
        ),
    ];

    for (edge, hit) in edge_strips {
        if hit.width() <= 0.0 || hit.height() <= 0.0 {
            continue;
        }
        let id = ui.id().with((&obj.id, format!("resize-edge-{edge:?}")));
        let resp = ui.interact(hit, id, Sense::click_and_drag());
        if resp.hovered() || resp.dragged() {
            interaction.hover_resize = Some(ResizeHandle::Edge(edge));
            ui.ctx().set_cursor_icon(resize_cursor(ResizeHandle::Edge(edge)));
        }
        if resp.clicked() {
            interaction.clicked = true;
        }
        if resp.dragged() {
            interaction.resize_drag = Some((
                ResizeHandle::Edge(edge),
                resp.drag_delta() / zoom,
            ));
            interaction.dragged = None;
        }
    }

    let corners = [
        (rect.left_top(), Corner::Nw),
        (rect.right_top(), Corner::Ne),
        (rect.left_bottom(), Corner::Sw),
        (rect.right_bottom(), Corner::Se),
    ];
    for (pos, corner) in corners {
        let hit = Rect::from_center_size(pos, Vec2::splat(cap * 2.0));
        let id = ui.id().with((&obj.id, format!("resize-corner-{corner:?}")));
        let resp = ui.interact(hit, id, Sense::click_and_drag());
        if resp.hovered() || resp.dragged() {
            interaction.hover_resize = Some(ResizeHandle::Corner(corner));
            ui.ctx().set_cursor_icon(resize_cursor(ResizeHandle::Corner(corner)));
        }
        if resp.clicked() {
            interaction.clicked = true;
        }
        if resp.dragged() {
            interaction.resize_drag = Some((
                ResizeHandle::Corner(corner),
                resp.drag_delta() / zoom,
            ));
            interaction.dragged = None;
        }
    }
}

fn resize_margin(zoom: f32) -> f32 {
    (12.0 / zoom.max(0.25)).max(8.0)
}

/// Classify pointer position into border resize zone (corner beats edge).
pub fn classify_resize_zone(pointer: Pos2, rect: Rect, zoom: f32) -> Option<ResizeHandle> {
    let margin = resize_margin(zoom);
    let near_top = (pointer.y - rect.min.y).abs() <= margin;
    let near_bottom = (pointer.y - rect.max.y).abs() <= margin;
    let near_left = (pointer.x - rect.min.x).abs() <= margin;
    let near_right = (pointer.x - rect.max.x).abs() <= margin;

    if near_top && near_left {
        return Some(ResizeHandle::Corner(Corner::Nw));
    }
    if near_top && near_right {
        return Some(ResizeHandle::Corner(Corner::Ne));
    }
    if near_bottom && near_left {
        return Some(ResizeHandle::Corner(Corner::Sw));
    }
    if near_bottom && near_right {
        return Some(ResizeHandle::Corner(Corner::Se));
    }

    let dist_top = (pointer.y - rect.min.y).abs();
    let dist_bottom = (pointer.y - rect.max.y).abs();
    let dist_left = (pointer.x - rect.min.x).abs();
    let dist_right = (pointer.x - rect.max.x).abs();
    let min_dist = dist_top
        .min(dist_bottom)
        .min(dist_left)
        .min(dist_right);

    if min_dist > margin {
        return None;
    }

    if min_dist == dist_top {
        Some(ResizeHandle::Edge(Edge::Top))
    } else if min_dist == dist_bottom {
        Some(ResizeHandle::Edge(Edge::Bottom))
    } else if min_dist == dist_left {
        Some(ResizeHandle::Edge(Edge::Left))
    } else {
        Some(ResizeHandle::Edge(Edge::Right))
    }
}

fn resize_cursor(handle: ResizeHandle) -> CursorIcon {
    match handle {
        ResizeHandle::Edge(Edge::Top) | ResizeHandle::Edge(Edge::Bottom) => {
            CursorIcon::ResizeVertical
        }
        ResizeHandle::Edge(Edge::Left) | ResizeHandle::Edge(Edge::Right) => {
            CursorIcon::ResizeHorizontal
        }
        ResizeHandle::Corner(Corner::Nw) => CursorIcon::ResizeNwSe,
        ResizeHandle::Corner(Corner::Ne) => CursorIcon::ResizeNeSw,
        ResizeHandle::Corner(Corner::Sw) => CursorIcon::ResizeNeSw,
        ResizeHandle::Corner(Corner::Se) => CursorIcon::ResizeNwSe,
    }
}

/// Apply a corner resize using world-space pointer delta.
pub fn apply_corner_resize(obj: &mut CanvasObject, corner: Corner, delta: Vec2) {
    ensure_card_dimensions(obj);
    match corner {
        Corner::Nw => {
            apply_edge_resize(obj, Edge::Left, delta);
            apply_edge_resize(obj, Edge::Top, delta);
        }
        Corner::Ne => {
            apply_edge_resize(obj, Edge::Right, delta);
            apply_edge_resize(obj, Edge::Top, delta);
        }
        Corner::Sw => {
            apply_edge_resize(obj, Edge::Left, delta);
            apply_edge_resize(obj, Edge::Bottom, delta);
        }
        Corner::Se => {
            apply_edge_resize(obj, Edge::Right, delta);
            apply_edge_resize(obj, Edge::Bottom, delta);
        }
    }
}

/// Apply resize for a unified border handle.
pub fn apply_resize_handle(obj: &mut CanvasObject, handle: ResizeHandle, delta: Vec2) {
    match handle {
        ResizeHandle::Corner(c) => apply_corner_resize(obj, c, delta),
        ResizeHandle::Edge(e) => apply_edge_resize(obj, e, delta),
    }
}

/// Ensure persisted w/h match what we draw when legacy cards stored 0.
pub fn ensure_card_dimensions(obj: &mut CanvasObject) {
    if obj.w <= 0.0 {
        obj.w = NOTE_CARD_WIDTH;
    }
    if obj.h <= 0.0 {
        obj.h = NOTE_CARD_HEIGHT;
    }
}

fn apply_edge_resize(obj: &mut CanvasObject, edge: Edge, delta: Vec2) {
    const MIN_CARD_SIZE: f32 = 120.0;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_rect() -> Rect {
        Rect::from_min_size(Pos2::new(100.0, 100.0), Vec2::new(200.0, 150.0))
    }

    #[test]
    fn classify_corner_nw() {
        let rect = test_rect();
        let p = Pos2::new(rect.min.x + 4.0, rect.min.y + 4.0);
        assert_eq!(
            classify_resize_zone(p, rect, 1.0),
            Some(ResizeHandle::Corner(Corner::Nw))
        );
    }

    #[test]
    fn classify_edge_top_midpoint() {
        let rect = test_rect();
        let p = Pos2::new(rect.center().x, rect.min.y + 6.0);
        assert_eq!(
            classify_resize_zone(p, rect, 1.0),
            Some(ResizeHandle::Edge(Edge::Top))
        );
    }

    #[test]
    fn classify_interior_none() {
        let rect = test_rect();
        let p = rect.center();
        assert_eq!(classify_resize_zone(p, rect, 1.0), None);
    }
}
