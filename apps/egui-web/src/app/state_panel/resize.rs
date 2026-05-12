use eframe::egui;

use crate::app::{QniApp, ResizeCorner, ResizeDrag};
use crate::constants::{
    STATE_VIEWPORT_MAX_HEIGHT, STATE_VIEWPORT_MAX_WIDTH, STATE_VIEWPORT_MIN_HEIGHT,
    STATE_VIEWPORT_MIN_WIDTH,
};
use crate::render::StatePanelLayout;

impl QniApp {
    /// Apply a resize drag step from the current pointer position. The
    /// dragged corner follows the cursor; the opposite corner stays fixed.
    /// Horizontal: the panel is centred, so we compensate `state_panel_offset.x`
    /// by `effective_dx / 2`. Vertical: the panel is bottom-anchored, so
    /// the top edges (TL/TR) need no adjustment, but the bottom edges
    /// (BL/BR) compensate by `effective_dy`.
    pub(crate) fn apply_resize_drag(&mut self, pointer: egui::Pos2) {
        let Some(drag) = self.state_panel.resize_drag else {
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
        let dh = if drag.corner.is_top() {
            -delta.y
        } else {
            delta.y
        };
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
        let eff_dy = if drag.corner.is_top() {
            -eff_dh
        } else {
            eff_dh
        };
        self.state_panel.viewport_size = egui::vec2(new_w, new_h);
        // Auto-centred horizontally → compensate by eff_dx/2 regardless of corner.
        self.state_panel.offset.x = drag.start_panel_offset.x + eff_dx / 2.0;
        // Bottom-anchored vertically → top corners need no offset; bottom
        // corners absorb the full eff_dy so the top edge stays put.
        self.state_panel.offset.y = if drag.corner.is_top() {
            drag.start_panel_offset.y
        } else {
            drag.start_panel_offset.y + eff_dy
        };
    }

    pub(crate) fn begin_resize_drag(&mut self, corner: ResizeCorner, pointer: egui::Pos2) {
        self.state_panel.resize_drag = Some(ResizeDrag {
            corner,
            start_pointer: pointer,
            start_viewport_size: self.state_panel.viewport_size,
            start_panel_offset: self.state_panel.offset,
        });
    }

    pub(crate) fn end_resize_drag(&mut self) {
        self.state_panel.resize_drag = None;
    }

    pub(crate) fn active_resize_corner(&self) -> Option<ResizeCorner> {
        self.state_panel.resize_drag.map(|d| d.corner)
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
        self.state_panel.hovered_resize_corner = None;
        for corner in [
            ResizeCorner::TopLeft,
            ResizeCorner::TopRight,
            ResizeCorner::BottomLeft,
            ResizeCorner::BottomRight,
        ] {
            let hit = QniApp::resize_handle_hit_rect(state_layout, self.state_panel.offset, corner);
            let id_label = match corner {
                ResizeCorner::TopLeft => "state_resize_tl",
                ResizeCorner::TopRight => "state_resize_tr",
                ResizeCorner::BottomLeft => "state_resize_bl",
                ResizeCorner::BottomRight => "state_resize_br",
            };
            let resp = ui.interact(hit, egui::Id::new(id_label), egui::Sense::drag());
            if resp.hovered() {
                self.state_panel.hovered_resize_corner = Some(corner);
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
