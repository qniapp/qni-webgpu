//! State-panel interactions (drag-to-move, viewport pan, Ctrl+wheel
//! zoom, aspect dims wheel/click, aspect popover, corner resize
//! handles) plus the small state helpers that back them.

use eframe::egui;

use super::{QniApp, ResizeCorner, ResizeDrag};
use crate::constants::{
    STATE_VIEWPORT_MAX_HEIGHT, STATE_VIEWPORT_MAX_WIDTH, STATE_VIEWPORT_MIN_HEIGHT,
    STATE_VIEWPORT_MIN_WIDTH,
};

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
}
