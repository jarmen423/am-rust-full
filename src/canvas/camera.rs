//! Infinite-canvas camera — converts between screen and world space.

use crate::model::CanvasCamera;

/// Camera state for the infinite canvas.
///
/// Stores a pan offset (in world space) and a zoom factor.
/// All screen↔world conversions assume the world origin (0,0) is at the
/// centre of the viewport rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// Pan offset in world space.
    pub offset: egui::Vec2,
    /// Zoom factor (1.0 = 100%).
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self::new()
    }
}

impl Camera {
    /// Create a new camera at origin with no zoom.
    pub fn new() -> Self {
        Self {
            offset: egui::Vec2::ZERO,
            zoom: 1.0,
        }
    }

    /// Convert a screen position to world space.
    ///
    /// Formula: `world = (screen - rect.center()) / zoom - offset`
    pub fn screen_to_world(&self, screen_pos: egui::Pos2, rect: &egui::Rect) -> egui::Pos2 {
        let center = rect.center();
        egui::Pos2::new(
            (screen_pos.x - center.x) / self.zoom - self.offset.x,
            (screen_pos.y - center.y) / self.zoom - self.offset.y,
        )
    }

    /// Convert a world position to screen space.
    ///
    /// Formula: `screen = (world + offset) * zoom + rect.center()`
    pub fn world_to_screen(&self, world_pos: egui::Pos2, rect: &egui::Rect) -> egui::Pos2 {
        let center = rect.center();
        egui::Pos2::new(
            (world_pos.x + self.offset.x) * self.zoom + center.x,
            (world_pos.y + self.offset.y) * self.zoom + center.y,
        )
    }

    /// Pan the camera by a screen-pixel delta.
    ///
    /// The delta is divided by zoom so that the camera moves the same
    /// *world-space* distance regardless of zoom level.
    pub fn pan(&mut self, delta: egui::Vec2) {
        self.offset.x += delta.x / self.zoom;
        self.offset.y += delta.y / self.zoom;
    }

    /// Zoom by `factor` (e.g. 1.1 or 0.9) keeping the world point under
    /// `screen_anchor` fixed on screen.
    pub fn zoom_at(&mut self, factor: f32, screen_anchor: egui::Pos2, rect: &egui::Rect) {
        let old_zoom = self.zoom;
        let new_zoom = (self.zoom * factor).clamp(0.1, 10.0);
        let center = rect.center();

        // Adjust offset so the world point under the cursor stays fixed.
        // Derivation:
        //   world = (screen_anchor - center) / old_zoom - old_offset
        //   world = (screen_anchor - center) / new_zoom - new_offset
        // Therefore:
        //   new_offset = old_offset + (screen_anchor - center) * (1/new_zoom - 1/old_zoom)
        self.offset += (screen_anchor.to_vec2() - center.to_vec2()) * (1.0 / new_zoom - 1.0 / old_zoom);
        self.zoom = new_zoom;
    }

    /// Convert to the model's [`CanvasCamera`] type for serialization.
    pub fn to_model_camera(&self) -> CanvasCamera {
        CanvasCamera {
            x: self.offset.x,
            y: self.offset.y,
            zoom: self.zoom,
        }
    }

    /// Restore camera state from a model [`CanvasCamera`].
    pub fn from_model_camera(cam: &CanvasCamera) -> Self {
        Self {
            offset: egui::vec2(cam.x, cam.y),
            zoom: cam.zoom.max(0.1).min(10.0),
        }
    }
}
