//! Theme tokens for the egui frontend.
//!
//! This module provides the shared colour palette and style helpers
//! used by all phases.  The backend (Phase 1) does **not** depend on
//! egui — it only re-exports the helpers so Phase 2+ can import them
//! from the shared crate.

/// RGB colour helpers for the Agentic Memory Workspace theme.
pub mod palette {
    /// Background — deep charcoal
    pub const BG_DARK: [u8; 3] = [0x1A, 0x1A, 0x1A];
    /// Surface — slightly lighter charcoal
    pub const BG_SURFACE: [u8; 3] = [0x24, 0x24, 0x24];
    /// Elevated surface — card/panel background
    pub const BG_ELEVATED: [u8; 3] = [0x2E, 0x2E, 0x2E];
    /// Primary accent — warm orange/amber
    pub const ACCENT_PRIMARY: [u8; 3] = [0xF0, 0xA0, 0x30];
    /// Secondary accent — muted teal
    pub const ACCENT_SECONDARY: [u8; 3] = [0x40, 0xB0, 0xA0];
    /// Text primary — off-white
    pub const TEXT_PRIMARY: [u8; 3] = [0xE8, 0xE8, 0xE8];
    /// Text secondary — muted grey
    pub const TEXT_SECONDARY: [u8; 3] = [0x90, 0x90, 0x90];
    /// Border — subtle divider
    pub const BORDER: [u8; 3] = [0x3A, 0x3A, 0x3A];
    /// Success — muted green
    pub const SUCCESS: [u8; 3] = [0x50, 0xC0, 0x60];
    /// Warning — amber
    pub const WARNING: [u8; 3] = [0xF0, 0xC0, 0x40];
    /// Error — muted red
    pub const ERROR: [u8; 3] = [0xE0, 0x50, 0x50];
}

/// Convert `[u8; 3]` to egui `Color32`.
///
/// This function is gated behind the `egui` feature so the backend
/// crate does not need to link egui.
#[cfg(feature = "egui")]
pub fn color32(rgb: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

/// Returns the full egui `Style` configured for the Agentic Memory
/// Workspace dark theme.
///
/// Gated behind the `egui` feature.
#[cfg(feature = "egui")]
pub fn agentic_style() -> egui::Style {
    use egui::{FontId, Style, TextStyle};
    use palette::*;

    let mut style = Style::default();

    // Use dark visuals
    style.visuals.dark_mode = true;
    style.visuals.window_fill = color32(BG_DARK);
    style.visuals.panel_fill = color32(BG_SURFACE);
    style.visuals.widgets.noninteractive.bg_fill = color32(BG_ELEVATED);
    style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, color32(TEXT_SECONDARY));
    style.visuals.widgets.inactive.bg_fill = color32(BG_ELEVATED);
    style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, color32(TEXT_PRIMARY));
    style.visuals.widgets.hovered.bg_fill = color32(ACCENT_PRIMARY);
    style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, color32(TEXT_PRIMARY));
    style.visuals.widgets.active.bg_fill = color32(ACCENT_SECONDARY);
    style.visuals.selection.bg_fill = color32(ACCENT_PRIMARY);
    style.visuals.selection.stroke = egui::Stroke::new(1.0, color32(TEXT_PRIMARY));

    // Typography
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(20.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Body,
        FontId::new(14.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(13.0, egui::FontFamily::Monospace),
    );
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(14.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(12.0, egui::FontFamily::Proportional),
    );

    style.spacing.window_margin = egui::Margin::same(12.0);
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);

    style
}
