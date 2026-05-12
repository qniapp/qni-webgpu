//! State-panel owned state extracted from `QniApp`.
//!
//! Geometry and interaction code still live in `app::state_panel` and
//! `render::state_panel_*`; this module keeps the mutable panel state in one
//! place so the app root does not own every field directly.

use eframe::egui;

use crate::constants::{
    state_circle_default_aspect_index, STATE_VIEWPORT_DEFAULT_HEIGHT, STATE_VIEWPORT_DEFAULT_WIDTH,
};

/// Which corner of the state panel a resize drag is anchored to. The
/// opposite corner stays fixed during the drag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResizeCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl ResizeCorner {
    pub(crate) fn is_top(self) -> bool {
        matches!(self, ResizeCorner::TopLeft | ResizeCorner::TopRight)
    }
}

/// In-flight resize drag. `start_*` fields snapshot the panel state at
/// drag-start so the new size is computed from total pointer movement,
/// avoiding integrator drift from per-frame deltas.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ResizeDrag {
    pub(crate) corner: ResizeCorner,
    pub(crate) start_pointer: egui::Pos2,
    pub(crate) start_viewport_size: egui::Vec2,
    pub(crate) start_panel_offset: egui::Vec2,
}

#[derive(Clone, Debug)]
pub(crate) struct StatePanelState {
    /// In-flight drag offset for moving the whole panel by its header strip.
    pub(crate) drag: Option<egui::Vec2>,
    /// Whole-panel offset from its auto bottom-centred layout position.
    pub(crate) offset: egui::Vec2,
    /// Pan offset of the circle grid inside the viewport.
    pub(crate) grid_offset: egui::Vec2,
    /// Zoom factor for the circle grid (1.0 = qni's natural cell sizes).
    /// Runtime clamps convert this to a rendered circle-size range.
    pub(crate) grid_zoom: f32,
    /// User-controlled circle viewport size below the header strip.
    pub(crate) viewport_size: egui::Vec2,
    /// In-flight resize drag (one of the four corner handles).
    pub(crate) resize_drag: Option<ResizeDrag>,
    /// Currently-hovered resize corner for cursor / paint state.
    pub(crate) hovered_resize_corner: Option<ResizeCorner>,
    /// Display-index (`row * cols + col`) of the hovered state-vector cell.
    pub(crate) hovered_cell: Option<u32>,
    /// `aspect_index = log2(cols)` for the state-vector circle grid.
    pub(crate) aspect_index: usize,
    /// Whether the user explicitly chose an aspect instead of following qni defaults.
    pub(crate) aspect_customized: bool,
    /// Whether the aspect-pick popover is open.
    pub(crate) aspect_popover_open: bool,
    /// Wheel accumulator for aspect stepping over the dimensions text.
    pub(crate) aspect_wheel_accum: f32,
}

impl Default for StatePanelState {
    fn default() -> Self {
        Self {
            drag: None,
            offset: egui::Vec2::ZERO,
            grid_offset: egui::Vec2::ZERO,
            grid_zoom: 1.0,
            viewport_size: egui::vec2(STATE_VIEWPORT_DEFAULT_WIDTH, STATE_VIEWPORT_DEFAULT_HEIGHT),
            resize_drag: None,
            hovered_resize_corner: None,
            hovered_cell: None,
            aspect_index: state_circle_default_aspect_index(1),
            aspect_customized: false,
            aspect_popover_open: false,
            aspect_wheel_accum: 0.0,
        }
    }
}
