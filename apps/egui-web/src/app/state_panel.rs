//! State-panel interactions (drag-to-move, viewport pan, Ctrl+wheel
//! zoom, aspect dims wheel/click, aspect popover, corner resize
//! handles) plus the small state helpers that back them.

use eframe::egui;

use super::{QniApp, ResizeCorner, ResizeDrag};
use crate::constants::{
    ASPECT_WHEEL_PER_STEP, MAX_QUBITS, STATE_GRID_ZOOM_MAX, STATE_GRID_ZOOM_MIN,
    STATE_VIEWPORT_MAX_HEIGHT, STATE_VIEWPORT_MAX_WIDTH, STATE_VIEWPORT_MIN_HEIGHT,
    STATE_VIEWPORT_MIN_WIDTH,
};
use crate::render::StatePanelLayout;
use crate::shared::amplitude_qubits;

impl QniApp {
    pub(crate) fn clamp_state_viewport_size(&mut self) {
        self.state_viewport_size.x = self
            .state_viewport_size
            .x
            .clamp(STATE_VIEWPORT_MIN_WIDTH, STATE_VIEWPORT_MAX_WIDTH);
        self.state_viewport_size.y = self
            .state_viewport_size
            .y
            .clamp(STATE_VIEWPORT_MIN_HEIGHT, STATE_VIEWPORT_MAX_HEIGHT);
    }

    /// Apply a resize drag step from the current pointer position. The
    /// dragged corner follows the cursor; the opposite corner stays fixed.
    /// Horizontal: the panel is centred, so we compensate `state_panel_offset.x`
    /// by `effective_dx / 2`. Vertical: the panel is bottom-anchored, so
    /// the top edges (TL/TR) need no adjustment, but the bottom edges
    /// (BL/BR) compensate by `effective_dy`.
    pub(crate) fn apply_resize_drag(&mut self, pointer: egui::Pos2) {
        let Some(drag) = self.state_resize_drag else {
            return;
        };
        let delta = pointer - drag.start_pointer;
        // Sign convention: positive `dw` / `dh` mean the panel grows when
        // the cursor moves in the natural direction for that corner.
        let dw = if matches!(
            drag.corner,
            ResizeCorner::TopLeft | ResizeCorner::BottomLeft
        ) {
            -delta.x
        } else {
            delta.x
        };
        let dh = if drag.corner.is_top() { -delta.y } else { delta.y };
        let new_w = (drag.start_viewport_size.x + dw)
            .clamp(STATE_VIEWPORT_MIN_WIDTH, STATE_VIEWPORT_MAX_WIDTH);
        let new_h = (drag.start_viewport_size.y + dh)
            .clamp(STATE_VIEWPORT_MIN_HEIGHT, STATE_VIEWPORT_MAX_HEIGHT);
        // After clamping the size, derive the *effective* delta — the
        // amount the dragged corner actually moved. This is what the
        // offset compensation has to match.
        let eff_dw = new_w - drag.start_viewport_size.x;
        let eff_dh = new_h - drag.start_viewport_size.y;
        let eff_dx = if matches!(
            drag.corner,
            ResizeCorner::TopLeft | ResizeCorner::BottomLeft
        ) {
            -eff_dw
        } else {
            eff_dw
        };
        let eff_dy = if drag.corner.is_top() { -eff_dh } else { eff_dh };
        self.state_viewport_size = egui::vec2(new_w, new_h);
        // Auto-centred horizontally → compensate by eff_dx/2 regardless of corner.
        self.state_panel_offset.x = drag.start_panel_offset.x + eff_dx / 2.0;
        // Bottom-anchored vertically → top corners need no offset; bottom
        // corners absorb the full eff_dy so the top edge stays put.
        self.state_panel_offset.y = if drag.corner.is_top() {
            drag.start_panel_offset.y
        } else {
            drag.start_panel_offset.y + eff_dy
        };
    }

    pub(crate) fn begin_resize_drag(&mut self, corner: ResizeCorner, pointer: egui::Pos2) {
        self.state_resize_drag = Some(ResizeDrag {
            corner,
            start_pointer: pointer,
            start_viewport_size: self.state_viewport_size,
            start_panel_offset: self.state_panel_offset,
        });
    }

    pub(crate) fn end_resize_drag(&mut self) {
        self.state_resize_drag = None;
    }

    pub(crate) fn active_resize_corner(&self) -> Option<ResizeCorner> {
        self.state_resize_drag.map(|d| d.corner)
    }

    /// Should wheel input over the panel area be captured (= not eaten
    /// by the surrounding `ScrollArea`)? True while the pointer is over
    /// the panel rect or any open aspect popover, so wheel events route
    /// to our dims-aspect / viewport-zoom handlers instead of scrolling
    /// the circuit underneath.
    pub(crate) fn compute_state_panel_input_gate(
        &self,
        ctx: &egui::Context,
        screen_rect: egui::Rect,
    ) -> bool {
        let state_count = self.state_count();
        let pre_state_layout = self.state_panel_layout(screen_rect, state_count);
        let pre_state_rect = pre_state_layout.state_rect.translate(self.state_panel_offset);
        let pre_popover_rect = if self.aspect_popover_open {
            let dims_hit = QniApp::dims_hit_rect(ctx, &pre_state_layout, self.state_panel_offset);
            let (rect, _) = QniApp::aspect_popover_layout(
                dims_hit,
                amplitude_qubits(state_count).clamp(1, MAX_QUBITS),
            );
            Some(rect)
        } else {
            None
        };
        ctx.input(|i| i.pointer.hover_pos())
            .map(|p| {
                pre_state_rect.contains(p)
                    || pre_popover_rect.map_or(false, |r| r.contains(p))
            })
            .unwrap_or(false)
    }

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
                self.state_panel_drag = Some(pos - handle_rect.min);
            }
        }
        if handle_response.dragged() {
            if let (Some(pos), Some(offset)) = (
                handle_response.interact_pointer_pos(),
                self.state_panel_drag,
            ) {
                let desired_min = pos - offset;
                self.state_panel_offset = desired_min - state_layout.state_rect.min;
                self.clamp_state_panel_offset(state_layout, screen_rect);
            }
        }
        if handle_response.drag_stopped() {
            self.state_panel_drag = None;
        }
    }

    /// Pan the circle grid inside the viewport (drag) + zoom the grid
    /// at the cursor anchor (Ctrl+wheel). The two share a single
    /// `ui.interact` on the viewport rect, so they live together.
    pub(crate) fn process_state_panel_viewport_pan_and_zoom(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        state_layout: &StatePanelLayout,
        screen_rect: egui::Rect,
        state_count: usize,
    ) {
        let viewport_rect = state_layout.viewport_rect.translate(self.state_panel_offset);
        let viewport_response = ui.interact(
            viewport_rect,
            egui::Id::new("state_panel_viewport"),
            egui::Sense::drag(),
        );
        if viewport_response.dragged() {
            self.state_grid_offset += viewport_response.drag_delta();
            self.clamp_state_grid_offset(state_layout);
        }

        // Cell hover detection: which (col, row) cell is the pointer
        // over? Maps the cursor's panel-local position to a display
        // index via the grid origin + cell pitch. Drives the GPU
        // shader's brightness-darken on fill / needle / outline.
        let new_hovered_cell = if viewport_response.hovered() {
            ctx.input(|i| i.pointer.hover_pos()).and_then(|pos| {
                let grid_origin = QniApp::grid_origin(
                    state_layout,
                    self.state_panel_offset,
                    self.state_grid_offset,
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
        if new_hovered_cell != self.hovered_state_cell {
            self.hovered_state_cell = new_hovered_cell;
            ctx.request_repaint();
        }

        // Ctrl+wheel inside the viewport zooms the grid. Plain wheel
        // is reserved for aspect-dims and (when over the panel) gets
        // routed there via `compute_state_panel_input_gate`. Zoom is
        // anchored at the cursor so the cell under it stays put.
        if viewport_response.hovered() {
            let scroll = ctx.input(|i| {
                if i.modifiers.ctrl || i.modifiers.command {
                    i.smooth_scroll_delta.y
                } else {
                    0.0
                }
            });
            if scroll.abs() > f32::EPSILON {
                let pointer = ctx.input(|i| i.pointer.hover_pos());
                let old_zoom = self.state_grid_zoom;
                let new_zoom = (old_zoom * (scroll * 0.005).exp())
                    .clamp(STATE_GRID_ZOOM_MIN, STATE_GRID_ZOOM_MAX);
                if (new_zoom - old_zoom).abs() > f32::EPSILON {
                    let anchor = pointer.unwrap_or(viewport_rect.center());
                    let pre_origin = QniApp::grid_origin(
                        state_layout,
                        self.state_panel_offset,
                        self.state_grid_offset,
                    );
                    let from_origin = anchor - pre_origin;
                    let scale = new_zoom / old_zoom;
                    let drift = from_origin * (scale - 1.0);
                    self.state_grid_zoom = new_zoom;
                    self.state_grid_offset -= drift;
                    // Layout recomputes next frame with the new zoom;
                    // clamp now to avoid a 1-frame out-of-bounds pan.
                    let zoomed = self.state_panel_layout(screen_rect, state_count);
                    self.clamp_state_grid_offset(&zoomed);
                }
            }
        }
    }

    /// Aspect dims text (right side of the strip): wheel accumulates
    /// into `aspect_wheel_accum` and steps the aspect each time the
    /// sum crosses ±`ASPECT_WHEEL_PER_STEP`; click toggles the popover.
    pub(crate) fn process_aspect_dims(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        aspect_qubits: usize,
        dims_hit: egui::Rect,
    ) {
        let dims_resp = ui.interact(
            dims_hit,
            egui::Id::new("state_dims"),
            egui::Sense::click(),
        );
        if dims_resp.hovered() {
            let plain_scroll = ctx.input(|i| {
                if i.modifiers.ctrl || i.modifiers.command {
                    0.0
                } else {
                    i.smooth_scroll_delta.y
                }
            });
            if plain_scroll.abs() > f32::EPSILON {
                self.aspect_wheel_accum += plain_scroll;
                let mut steps: i32 = 0;
                while self.aspect_wheel_accum >= ASPECT_WHEEL_PER_STEP {
                    self.aspect_wheel_accum -= ASPECT_WHEEL_PER_STEP;
                    steps -= 1; // positive scroll → taller (cols −1)
                }
                while self.aspect_wheel_accum <= -ASPECT_WHEEL_PER_STEP {
                    self.aspect_wheel_accum += ASPECT_WHEEL_PER_STEP;
                    steps += 1; // negative scroll → wider (cols +1)
                }
                if steps != 0 {
                    let new_aspect = (self.aspect_index as i32 + steps)
                        .clamp(0, aspect_qubits as i32) as usize;
                    if new_aspect != self.aspect_index {
                        self.aspect_index = new_aspect;
                        self.aspect_customized = true;
                        ctx.request_repaint();
                    }
                }
            } else {
                // Wheel stopped this frame — drop any sub-step residue
                // so the next notch starts from zero.
                self.aspect_wheel_accum = 0.0;
            }
            ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
        } else {
            // Pointer left the dims area — discard pending accum so
            // re-entering doesn't fire a stale step.
            self.aspect_wheel_accum = 0.0;
        }
        if dims_resp.clicked() {
            self.aspect_popover_open = !self.aspect_popover_open;
        }
    }

    /// Aspect popover row clicks + outside-click / ESC close.
    pub(crate) fn process_aspect_popover(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        aspect_qubits: usize,
        dims_hit: egui::Rect,
    ) {
        if self.aspect_popover_open {
            let (popover_rect, row_rects) =
                QniApp::aspect_popover_layout(dims_hit, aspect_qubits);
            for (i, row_rect) in row_rects.iter().enumerate() {
                let resp = ui.interact(
                    *row_rect,
                    egui::Id::new(("state_aspect_row", i)),
                    egui::Sense::click(),
                );
                if resp.hovered() {
                    ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    self.aspect_index = i;
                    self.aspect_customized = true;
                    self.aspect_popover_open = false;
                }
            }
            // Outside click closes. `any_pressed` catches a click that
            // initiated outside the popover this frame.
            let pressed = ctx.input(|i| i.pointer.any_pressed());
            if pressed {
                if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                    if !dims_hit.contains(pos) && !popover_rect.contains(pos) {
                        self.aspect_popover_open = false;
                    }
                }
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && self.aspect_popover_open {
            self.aspect_popover_open = false;
        }
    }

    /// Four corner resize handles — hover/cursor + drag start/move/end.
    /// Registered last so they take priority over the strip / viewport
    /// interacts for overlapping pointer hits.
    pub(crate) fn process_resize_handles(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        state_layout: &StatePanelLayout,
    ) {
        self.hovered_resize_corner = None;
        for corner in [
            ResizeCorner::TopLeft,
            ResizeCorner::TopRight,
            ResizeCorner::BottomLeft,
            ResizeCorner::BottomRight,
        ] {
            let hit = QniApp::resize_handle_hit_rect(
                state_layout,
                self.state_panel_offset,
                corner,
            );
            let id_label = match corner {
                ResizeCorner::TopLeft => "state_resize_tl",
                ResizeCorner::TopRight => "state_resize_tr",
                ResizeCorner::BottomLeft => "state_resize_bl",
                ResizeCorner::BottomRight => "state_resize_br",
            };
            let resp = ui.interact(hit, egui::Id::new(id_label), egui::Sense::drag());
            if resp.hovered() {
                self.hovered_resize_corner = Some(corner);
            }
            if resp.drag_started() {
                if let Some(p) = resp.interact_pointer_pos() {
                    self.begin_resize_drag(corner, p);
                }
            }
            if resp.dragged() && self.active_resize_corner() == Some(corner) {
                if let Some(p) = resp.interact_pointer_pos() {
                    self.apply_resize_drag(p);
                }
            }
            if resp.drag_stopped() && self.active_resize_corner() == Some(corner) {
                self.end_resize_drag();
            }
            if resp.hovered() || self.active_resize_corner() == Some(corner) {
                let cursor = match corner {
                    ResizeCorner::TopLeft | ResizeCorner::BottomRight => {
                        egui::CursorIcon::ResizeNwSe
                    }
                    ResizeCorner::TopRight | ResizeCorner::BottomLeft => {
                        egui::CursorIcon::ResizeNeSw
                    }
                };
                ctx.set_cursor_icon(cursor);
            }
        }
    }
}
