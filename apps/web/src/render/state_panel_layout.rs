//! State-vector panel geometry facade.
//!
//! Submodules own panel sizing/origin, aspect popover rows, hit rects, and
//! offset clamps. Pure geometry, no `Painter` calls.

mod aspect;
mod clamp;
mod geometry;
mod hit_rects;

use eframe::egui;

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
}

impl StatePanelLayout {
    /// Cell-to-cell pitch in panel-local pixels: the distance between two
    /// adjacent cell origins. Used by hover-test code to map a cursor
    /// position to its `(col, row)` cell.
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
