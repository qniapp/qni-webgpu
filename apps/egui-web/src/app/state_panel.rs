//! State-panel interaction facade.
//!
//! Submodules own drag-to-move, viewport pan/zoom, aspect selector, and corner
//! resize handles. The small shared helpers stay here.

mod aspect;
mod drag;
mod resize;
mod viewport;

use eframe::egui;

use super::QniApp;
use crate::constants::{
    MAX_QUBITS, STATE_VIEWPORT_MAX_HEIGHT, STATE_VIEWPORT_MAX_WIDTH, STATE_VIEWPORT_MIN_HEIGHT,
    STATE_VIEWPORT_MIN_WIDTH,
};
use crate::shared::amplitude_qubits;

impl QniApp {
    pub(crate) fn clamp_state_viewport_size(&mut self) {
        self.state_panel.viewport_size.x = self
            .state_panel
            .viewport_size
            .x
            .clamp(STATE_VIEWPORT_MIN_WIDTH, STATE_VIEWPORT_MAX_WIDTH);
        self.state_panel.viewport_size.y = self
            .state_panel
            .viewport_size
            .y
            .clamp(STATE_VIEWPORT_MIN_HEIGHT, STATE_VIEWPORT_MAX_HEIGHT);
    }

    /// Should pointer input over the panel area be captured (= not seen
    /// by the circuit underneath)? True while the pointer is over the
    /// panel rect or any open aspect popover, so hover/click/wheel events
    /// route to state-panel handlers instead of circuit step preview / scroll.
    pub(crate) fn compute_state_panel_input_gate(
        &self,
        ctx: &egui::Context,
        screen_rect: egui::Rect,
    ) -> bool {
        if self.state_panel.drag.is_some() || self.state_panel.resize_drag.is_some() {
            return true;
        }

        let state_count = self.state_count();
        let pre_state_layout = self.state_panel_layout(screen_rect, state_count);
        let pre_state_rect = pre_state_layout
            .state_rect
            .translate(self.state_panel.offset);
        let pre_popover_rect = if self.state_panel.aspect_popover_open {
            let dims_hit = QniApp::dims_hit_rect(ctx, &pre_state_layout, self.state_panel.offset);
            let (rect, _) = QniApp::aspect_popover_layout(
                dims_hit,
                amplitude_qubits(state_count).clamp(1, MAX_QUBITS),
            );
            Some(rect)
        } else {
            None
        };
        ctx.input(|i| i.pointer.hover_pos())
            .map(|p| pre_state_rect.contains(p) || pre_popover_rect.is_some_and(|r| r.contains(p)))
            .unwrap_or(false)
    }
}
