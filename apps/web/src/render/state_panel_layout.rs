//! State-vector panel geometry facade.
//!
//! Submodules own panel sizing/origin, aspect popover rows, hit rects, and
//! offset clamps. Pure geometry, no `Painter` calls.

mod aspect;
mod clamp;
mod geometry;
mod hit_rects;

use eframe::egui;

pub(crate) use geometry::vertical_default_aspect_index;

pub(crate) struct StatePanelLayout {
    pub(super) state_count: usize,
    pub(super) qubits: usize,
    pub(super) columns: usize,
    pub(super) size: f32,
    pub(super) gap: f32,
    pub(super) radius: f32,
    pub(super) stroke: f32,
    pub(super) inner_radius: f32,
    /// Total pixel size of the circle grid (cols × cell_pitch, rows × cell_pitch).
    pub(super) grid_size: egui::Vec2,
    /// Inner area below the header strip where circles render. Fixed size;
    /// when the grid is smaller it gets centred, when larger it pans inside.
    pub(crate) viewport_rect: egui::Rect,
    pub(crate) state_rect: egui::Rect,
    pub(crate) handle_height: f32,
    pub(crate) show_restart_button: bool,
}

impl StatePanelLayout {
    /// 32px: a 24px button and 8px label gap.
    pub(crate) fn restart_space(&self) -> f32 {
        if self.show_restart_button {
            crate::tailwind::SPACING_8
        } else {
            0.0
        }
    }

    pub(crate) fn restart_rect(&self, offset: egui::Vec2) -> egui::Rect {
        let state_rect = self.state_rect.translate(offset);
        let padding = crate::constants::STATE_PANEL_CORNER_RADIUS + 6.0;
        egui::Rect::from_center_size(
            egui::pos2(
                state_rect.right() - padding - 12.0,
                state_rect.top() + self.handle_height / 2.0,
            ),
            egui::vec2(crate::tailwind::SPACING_6, crate::tailwind::SPACING_6),
        )
    }

    /// Cell-to-cell pitch in panel-local pixels: the distance between two
    /// adjacent cell origins. Used by hover-test code to map a cursor
    /// position to its `(col, row)` cell.
    /// Keep the fitted overview reachable, but allow zooming up to 256px cells.
    pub(crate) fn zoom_limits(&self, current_zoom: f32) -> (f32, f32) {
        let (min, max) = crate::constants::state_grid_zoom_limits(self.size / current_zoom);
        (min.min(1.0), max)
    }

    pub(crate) fn cell_pitch(&self) -> f32 {
        self.size + self.gap
    }

    pub(crate) fn columns(&self) -> usize {
        self.columns
    }

    pub(crate) fn rows(&self) -> usize {
        self.state_count / self.columns.max(1)
    }
}
