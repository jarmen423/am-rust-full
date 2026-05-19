//! Neutral local workspace diagnostics (not OpenClaw metrics).

use crate::api::{Promise, SharedPromise};
use crate::model::DiagnosticsHealthResponse;
use egui::{RichText, Ui};

#[derive(Debug, Default)]
pub struct DiagnosticsPanelState {
    pub health: Option<DiagnosticsHealthResponse>,
    pub last_ping_attempt_id: Option<String>,
    pub status_message: Option<String>,
}

pub struct DiagnosticsPanelOutput {
    pub refresh_health: bool,
    pub ping_diagnostics: bool,
}

impl Default for DiagnosticsPanelOutput {
    fn default() -> Self {
        Self {
            refresh_health: false,
            ping_diagnostics: false,
        }
    }
}

pub fn show(ui: &mut Ui, state: &mut DiagnosticsPanelState) -> DiagnosticsPanelOutput {
    let mut out = DiagnosticsPanelOutput::default();
    ui.heading("Workspace diagnostics");
    ui.label(
        RichText::new("Local runtime health — not hosted OpenClaw metrics.")
            .small()
            .color(ui.visuals().weak_text_color()),
    );
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.button("Refresh").clicked() {
            out.refresh_health = true;
        }
        if ui.button("Ping channel").clicked() {
            out.ping_diagnostics = true;
        }
    });
    ui.add_space(8.0);
    if let Some(ref h) = state.health {
        ui.label(format!("Ladybug: {}", if h.ladybug_available { "up" } else { "down" }));
        ui.label(format!("Active attempts: {}", h.active_attempts));
        ui.label(format!("Workspace: {}", h.workspace_id));
    } else {
        ui.label("Health not loaded yet.");
    }
    if let Some(ref id) = state.last_ping_attempt_id {
        ui.label(format!("Last ping attempt: {id}"));
    }
    if let Some(ref msg) = state.status_message {
        ui.label(RichText::new(msg).color(ui.visuals().warn_fg_color));
    }
    out
}

pub fn poll_health(
    promise: &SharedPromise<DiagnosticsHealthResponse>,
    state: &mut DiagnosticsPanelState,
) {
    let mut lock = promise.lock();
    if let Some(h) = lock.take() {
        state.health = Some(h);
        state.status_message = Some("Health refreshed.".to_string());
    } else if let Promise::Failed(e) = std::mem::replace(&mut *lock, Promise::Idle) {
        state.status_message = Some(format!("Health failed: {e}"));
    }
}

pub fn poll_ping(promise: &SharedPromise<String>, state: &mut DiagnosticsPanelState) {
    let mut lock = promise.lock();
    if let Some(id) = lock.take() {
        state.last_ping_attempt_id = Some(id);
        state.status_message = Some("Diagnostics ping OK.".to_string());
    } else if let Promise::Failed(e) = std::mem::replace(&mut *lock, Promise::Idle) {
        state.status_message = Some(format!("Ping failed: {e}"));
    }
}
