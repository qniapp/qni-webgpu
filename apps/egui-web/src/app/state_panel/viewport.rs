use eframe::egui;

use crate::app::QniApp;
use crate::constants::{state_circle_layout, state_grid_zoom_limits, MAX_QUBITS};
use crate::render::StatePanelLayout;
use crate::shared::amplitude_qubits;

impl QniApp {
    /// Pan the circle grid inside the viewport (drag) + zoom the grid
    /// at the cursor anchor (wheel, Google-Maps style). The two share a
    /// single `ui.interact` on the viewport rect, so they live together.
    pub(crate) fn process_state_panel_viewport_pan_and_zoom(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        state_layout: &StatePanelLayout,
        screen_rect: egui::Rect,
        state_count: usize,
    ) {
        let viewport_rect = state_layout
            .viewport_rect
            .translate(self.state_panel.offset);
        let viewport_response = ui.interact(
            viewport_rect,
            egui::Id::new("state_panel_viewport"),
            egui::Sense::drag(),
        );
        if viewport_response.dragged() {
            self.state_panel.grid_offset += viewport_response.drag_delta();
            self.clamp_state_grid_offset(state_layout);
        }

        self.update_state_cell_hover(ctx, &viewport_response, state_layout);
        self.zoom_state_grid_at_cursor(
            ctx,
            &viewport_response,
            state_layout,
            screen_rect,
            state_count,
        );
    }

    fn update_state_cell_hover(
        &mut self,
        ctx: &egui::Context,
        viewport_response: &egui::Response,
        state_layout: &StatePanelLayout,
    ) {
        // Cell hover detection: which (col, row) cell is the pointer
        // over? Maps the cursor's panel-local position to a display
        // index via the grid origin + cell pitch. Drives the GPU
        // shader's brightness-darken on fill / needle / outline.
        // While any drag is in progress, hide the popup/highlight so the
        // floating card doesn't chase the pointer or obscure the drag target.
        let suppress_hover = self.dragging.is_some()
            || self.state_panel.drag.is_some()
            || self.state_panel.resize_drag.is_some()
            || ctx.input(|i| i.pointer.primary_down());
        let new_hovered_cell = if !suppress_hover && viewport_response.hovered() {
            ctx.input(|i| i.pointer.hover_pos()).and_then(|pos| {
                let grid_origin = QniApp::grid_origin(
                    state_layout,
                    self.state_panel.offset,
                    self.state_panel.grid_offset,
                );
                let local = pos - grid_origin;
                let pitch = state_layout.cell_pitch();
                if pitch <= 0.0 || local.x < 0.0 || local.y < 0.0 {
                    return None;
                }
                let col = (local.x / pitch) as usize;
                let row = (local.y / pitch) as usize;
                if col >= state_layout.columns() || row >= state_layout.rows() {
                    return None;
                }
                Some((row * state_layout.columns() + col) as u32)
            })
        } else {
            None
        };
        if new_hovered_cell != self.state_panel.hovered_cell {
            self.state_panel.hovered_cell = new_hovered_cell;
            ctx.request_repaint();
        }
    }

    fn zoom_state_grid_at_cursor(
        &mut self,
        ctx: &egui::Context,
        viewport_response: &egui::Response,
        state_layout: &StatePanelLayout,
        screen_rect: egui::Rect,
        state_count: usize,
    ) {
        // Wheel inside the viewport zooms the grid, anchored at the cursor
        // so the cell under it stays put. The dimensions text in the header
        // owns wheel events for aspect changes; the viewport gets the
        // Google-Maps-style plain-wheel zoom path.
        if viewport_response.hovered() {
            let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > f32::EPSILON {
                let pointer = ctx.input(|i| i.pointer.hover_pos());
                let old_zoom = self.state_panel.grid_zoom;
                let qubits = amplitude_qubits(state_count).clamp(1, MAX_QUBITS);
                let natural_circle_size =
                    state_circle_layout(qubits, self.state_panel.aspect_index).size;
                let (min_zoom, max_zoom) = state_grid_zoom_limits(natural_circle_size);
                let new_zoom = (old_zoom * (scroll * 0.005).exp()).clamp(min_zoom, max_zoom);
                if (new_zoom - old_zoom).abs() > f32::EPSILON {
                    let viewport_rect = state_layout
                        .viewport_rect
                        .translate(self.state_panel.offset);
                    let anchor = pointer.unwrap_or(viewport_rect.center());
                    let pre_origin = QniApp::grid_origin(
                        state_layout,
                        self.state_panel.offset,
                        self.state_panel.grid_offset,
                    );
                    let from_origin = anchor - pre_origin;
                    let scale = new_zoom / old_zoom;
                    let desired_origin = anchor - from_origin * scale;
                    self.state_panel.grid_zoom = new_zoom;
                    // Convert the desired post-zoom origin through the zoomed
                    // layout's own pan base. This keeps the cursor anchor fixed
                    // even when the grid switches between centred-fit and
                    // overflowing-panned modes.
                    let zoomed = self.state_panel_layout(screen_rect, state_count);
                    self.state_panel.grid_offset = QniApp::grid_offset_for_origin(
                        &zoomed,
                        self.state_panel.offset,
                        desired_origin,
                    );
                    self.clamp_state_grid_offset(&zoomed);
                    ctx.request_repaint();
                }
            }
        }
    }
}
