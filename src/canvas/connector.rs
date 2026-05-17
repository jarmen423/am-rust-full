//! Connector rendering — cubic Bezier curves with arrowheads between cards.

use crate::canvas::camera::Camera;
use crate::model::{CanvasConnector, CanvasObject};
use crate::theme::color32;
use crate::theme::palette;
use egui::epaint::{CubicBezierShape, QuadraticBezierShape};
use egui::{Color32, Pos2, Shape, Stroke, Ui, Vec2};

/// Render a connector between two cards as a cubic Bezier curve.
///
/// The curve runs from the midpoint of the source card's nearest edge to
/// the midpoint of the target card's nearest edge.  Control points are
/// offset perpendicular to the dominant direction of the connection.
/// An arrowhead is drawn at the target end.
pub fn render_connector(
    ui: &mut Ui,
    conn: &CanvasConnector,
    from_obj: &CanvasObject,
    to_obj: &CanvasObject,
    camera: &Camera,
) {
    let rect = ui.max_rect();

    // ── Source and target centres in screen space ─────────────────────
    let from_screen = camera.world_to_screen(Pos2::new(from_obj.x, from_obj.y), &rect);
    let to_screen = camera.world_to_screen(Pos2::new(to_obj.x, to_obj.y), &rect);

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
    let to_w = if to_obj.w > 0.0 { to_obj.w } else { crate::model::NOTE_CARD_WIDTH };
    let to_h = if to_obj.h > 0.0 { to_obj.h } else { crate::model::NOTE_CARD_HEIGHT };

    // Card rects in screen space
    let from_rect = egui::Rect::from_min_size(
        from_screen,
        Vec2::new(from_w * camera.zoom, from_h * camera.zoom),
    );
    let to_rect = egui::Rect::from_min_size(
        to_screen,
        Vec2::new(to_w * camera.zoom, to_h * camera.zoom),
    );

    // ── Pick the best edge midpoints ──────────────────────────────────
    let (start, end) = best_edge_midpoints(from_rect, to_rect);

    // ── Bezier control points ─────────────────────────────────────────
    let dir = end - start;
    let dist = dir.length();
    let ctrl_offset = dist * 0.4;

    let (cp1, cp2) = if dir.x.abs() > dir.y.abs() {
        // Horizontally dominant — offset control points vertically
        let dy = ctrl_offset * 0.6 * dir.y.signum();
        (
            Pos2::new(start.x + dir.x.abs() * 0.3, start.y + dy),
            Pos2::new(end.x - dir.x.abs() * 0.3, end.y - dy),
        )
    } else {
        // Vertically dominant — offset control points horizontally
        let dx = ctrl_offset * 0.6 * dir.x.signum();
        (
            Pos2::new(start.x + dx, start.y + dir.y.abs() * 0.3),
            Pos2::new(end.x - dx, end.y - dir.y.abs() * 0.3),
        )
    };

    // ── Stroke ────────────────────────────────────────────────────────
    let stroke = Stroke::new(2.0, color32(palette::ACCENT_SECONDARY));

    // ── Draw cubic Bezier ─────────────────────────────────────────────
    ui.painter().add(Shape::CubicBezier(CubicBezierShape::from_points_stroke(
        [start, cp1, cp2, end],
        false,
        Color32::TRANSPARENT,
        stroke,
    )));

    // ── Arrowhead at target ───────────────────────────────────────────
    // Sample a point slightly before the end to get the tangent direction
    let t = 0.95;
    let sample = cubic_bezier_point(start, cp1, cp2, end, t);
    let arrow_dir = (end - sample).normalized();

    draw_arrowhead(ui.painter(), end, arrow_dir, 8.0 * camera.zoom, color32(palette::ACCENT_SECONDARY));

    // ── Optional label ────────────────────────────────────────────────
    if !conn.label.is_empty() {
        let mid = cubic_bezier_point(start, cp1, cp2, end, 0.5);
        let label_size = (10.0 * camera.zoom).max(7.0);
        let galley = ui.painter().layout(
            conn.label.clone(),
            egui::FontId::proportional(label_size),
            color32(palette::TEXT_SECONDARY),
            200.0,
        );
        let label_pos = mid - galley.rect.size() * 0.5;
        ui.painter().galley(label_pos, galley, egui::Color32::WHITE);
    }
}

/// Render a preview line while the user is dragging a new connection.
///
/// Both `from_pos` and `to_pos` are in **screen** space.
pub fn render_connection_preview(
    ui: &mut Ui,
    from_pos: Pos2,
    to_pos: Pos2,
    _camera: &Camera,
) {
    let stroke = Stroke::new(1.5, color32(palette::ACCENT_SECONDARY));

    // Simple quadratic curve for the preview
    let mid = (from_pos + to_pos.to_vec2()) * 0.5;
    let dir = to_pos - from_pos;
    let ctrl = if dir.x.abs() > dir.y.abs() {
        // Offset perpendicular (vertical)
        let offset = dir.x.abs() * 0.3;
        Pos2::new(mid.x, mid.y + offset * dir.y.signum())
    } else {
        // Offset perpendicular (horizontal)
        let offset = dir.y.abs() * 0.3;
        Pos2::new(mid.x + offset * dir.x.signum(), mid.y)
    };

    ui.painter().add(Shape::QuadraticBezier(QuadraticBezierShape::from_points_stroke(
        [from_pos, ctrl, to_pos],
        false,
        Color32::TRANSPARENT,
        stroke,
    )));

    // Small circle at source
    ui.painter().circle_filled(from_pos, 4.0, color32(palette::ACCENT_SECONDARY));

    // Target arrowhead
    let arrow_dir = (to_pos - ctrl).normalized();
    draw_arrowhead(ui.painter(), to_pos, arrow_dir, 6.0, color32(palette::ACCENT_SECONDARY));
}

// ═══════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════

/// Compute the best pair of edge midpoints to connect two rectangles.
///
/// Chooses the combination that minimises the distance between the
/// midpoints while preferring natural facing edges.
fn best_edge_midpoints(from: egui::Rect, to: egui::Rect) -> (Pos2, Pos2) {
    let from_top = Pos2::new(from.center().x, from.min.y);
    let from_bottom = Pos2::new(from.center().x, from.max.y);
    let from_left = Pos2::new(from.min.x, from.center().y);
    let from_right = Pos2::new(from.max.x, from.center().y);

    let to_top = Pos2::new(to.center().x, to.min.y);
    let to_bottom = Pos2::new(to.center().x, to.max.y);
    let to_left = Pos2::new(to.min.x, to.center().y);
    let to_right = Pos2::new(to.max.x, to.center().y);

    let from_edges = [from_top, from_bottom, from_left, from_right];
    let to_edges = [to_top, to_bottom, to_left, to_right];

    let mut best_pair = (from_right, to_left);
    let mut best_dist = f32::INFINITY;

    for &fe in &from_edges {
        for &te in &to_edges {
            let d = fe.distance(te);
            if d < best_dist {
                best_dist = d;
                best_pair = (fe, te);
            }
        }
    }

    best_pair
}

/// Evaluate a cubic Bezier at parameter `t` (0..1).
fn cubic_bezier_point(p0: Pos2, p1: Pos2, p2: Pos2, p3: Pos2, t: f32) -> Pos2 {
    let u = 1.0 - t;
    let u2 = u * u;
    let u3 = u2 * u;
    let t2 = t * t;
    let t3 = t2 * t;

    Pos2::new(
        u3 * p0.x + 3.0 * u2 * t * p1.x + 3.0 * u * t2 * p2.x + t3 * p3.x,
        u3 * p0.y + 3.0 * u2 * t * p1.y + 3.0 * u * t2 * p2.y + t3 * p3.y,
    )
}

/// Draw a small filled triangle arrowhead.
fn draw_arrowhead(
    painter: &egui::Painter,
    tip: Pos2,
    dir: Vec2,
    size: f32,
    color: egui::Color32,
) {
    let perp = Vec2::new(-dir.y, dir.x);
    let base = tip - dir * size;
    let left = base + perp * size * 0.5;
    let right = base - perp * size * 0.5;

    painter.add(egui::Shape::convex_polygon(
        vec![tip, left, right],
        color,
        Stroke::NONE,
    ));
}
