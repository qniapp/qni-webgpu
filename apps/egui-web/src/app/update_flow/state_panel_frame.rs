use eframe::egui;

use super::StatePanelFrameState;
use crate::app::QniApp;
use crate::constants::{
    state_circle_default_aspect_index, state_circle_layout, state_grid_zoom_limits, MAX_QUBITS,
};
use crate::shared::amplitude_qubits;

impl QniApp {
    pub(super) fn prepare_state_panel_frame(
        &mut self,
        screen_rect: egui::Rect,
    ) -> StatePanelFrameState {
        // Resolve the state count / aspect / layout for this frame. While a gate
        // is mid-drag, an extra phantom qubit is added (`drag_state_count`) so
        // the layout doesn't reflow underneath the user during the drag.
        let base_state_count = self.state_count();
        let state_count = if self.dragging.is_some() {
            self.drag_state_count.unwrap_or(base_state_count)
        } else {
            base_state_count
        };
        let recompute = self.gpu_plan.needs_recompute_for(state_count);
        self.clamp_state_viewport_size();

        // Sync aspect_index with the current qubit count. While the user hasn't
        // customised, follow qni's per-qubit default; once customised, only clamp
        // to the valid [0, qubits] range so the choice stays sticky.
        let aspect_qubits = amplitude_qubits(state_count).clamp(1, MAX_QUBITS);
        if !self.state_panel.aspect_customized {
            self.state_panel.aspect_index = state_circle_default_aspect_index(aspect_qubits);
        } else {
            self.state_panel.aspect_index = self.state_panel.aspect_index.min(aspect_qubits);
        }
        let natural_circle_size =
            state_circle_layout(aspect_qubits, self.state_panel.aspect_index).size;
        let (min_zoom, max_zoom) = state_grid_zoom_limits(natural_circle_size);
        self.state_panel.grid_zoom = self.state_panel.grid_zoom.clamp(min_zoom, max_zoom);

        let layout = self.state_panel_layout(screen_rect, state_count);
        self.clamp_state_panel_offset(&layout, screen_rect);
        self.clamp_state_grid_offset(&layout);

        StatePanelFrameState {
            state_count,
            recompute,
            layout,
        }
    }

    pub(super) fn process_state_panel_interactions(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        screen_rect: egui::Rect,
        state_frame: &StatePanelFrameState,
    ) {
        let state_rect = state_frame
            .layout
            .state_rect
            .translate(self.state_panel.offset);
        let handle_rect = egui::Rect::from_min_size(
            state_rect.min,
            egui::vec2(
                state_rect.width(),
                state_frame.layout.handle_height.max(6.0),
            ),
        );

        // Order matters: resize handles are registered last so they take
        // priority over the strip and viewport interacts at overlapping hits.
        self.process_state_panel_strip_drag(ui, &state_frame.layout, screen_rect, handle_rect);
        self.process_state_panel_viewport_pan_and_zoom(
            ctx,
            ui,
            &state_frame.layout,
            screen_rect,
            state_frame.state_count,
        );
        let dims_hit = QniApp::dims_hit_rect(ctx, &state_frame.layout, self.state_panel.offset);
        let aspect_qubits = amplitude_qubits(state_frame.state_count).clamp(1, MAX_QUBITS);
        self.process_aspect_dims(ctx, ui, aspect_qubits, dims_hit);
        self.process_aspect_popover(ctx, ui, aspect_qubits, dims_hit);
        self.process_resize_handles(ctx, ui, &state_frame.layout);
    }
}
