use eframe::egui;

use crate::app::QniApp;
use crate::render::StatePanelLayout;

impl QniApp {
    /// Drag-to-move the panel via the header strip. The corner-handle
    /// areas are excluded so dragging from a corner is interpreted as
    /// resize (handled separately).
    pub(crate) fn process_state_panel_strip_drag(
        &mut self,
        ui: &mut egui::Ui,
        state_layout: &StatePanelLayout,
        screen_rect: egui::Rect,
        handle_rect: egui::Rect,
    ) {
        const STRIP_CORNER_EXCLUDE: f32 = 16.0;
        let strip_drag_rect = egui::Rect::from_min_max(
            handle_rect.min + egui::vec2(STRIP_CORNER_EXCLUDE, 0.0),
            handle_rect.max - egui::vec2(STRIP_CORNER_EXCLUDE, 0.0),
        );
        let handle_response = ui.interact(
            strip_drag_rect,
            egui::Id::new("state_panel_handle"),
            egui::Sense::drag(),
        );
        if handle_response.drag_started() {
            if let Some(pos) = handle_response.interact_pointer_pos() {
                self.state_panel.drag = Some(pos - handle_rect.min);
            }
        }
        if handle_response.dragged() {
            if let (Some(pos), Some(offset)) = (
                handle_response.interact_pointer_pos(),
                self.state_panel.drag,
            ) {
                let desired_min = pos - offset;
                self.state_panel.offset = desired_min - state_layout.state_rect.min;
                self.clamp_state_panel_offset(state_layout, screen_rect);
            }
        }
        if handle_response.drag_stopped() {
            self.state_panel.drag = None;
        }
    }
}
