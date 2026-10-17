//! Gate drag controller facade.
//!
//! The submodules keep qni's SNAP / DROP / RESIZE style event boundaries
//! explicit while `gate_input` stays a thin egui adapter.

mod circuit_block;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod circuit_block_tests;
mod drop;
mod hover;
mod preview;
mod resize;
mod scroll;
mod start;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;

use eframe::egui;

use super::{CircuitColumnIndex, QniApp};
use crate::constants::{GATE_SIZE, PALETTE_ROW_Y, SLOT_SPACING, STEP_OUTSET_Y};
use crate::layout::{
    layout_metrics, nearest_slot_index, palette_start_x, LayoutMetrics, Palette, PaletteLayout,
};

#[derive(Clone, Copy, Debug)]
pub(super) struct DragPointer {
    pub(super) screen_pos: Option<egui::Pos2>,
    pub(super) local_pos: Option<egui::Pos2>,
    pub(super) down: bool,
    pub(super) start: bool,
    pub(super) shift_at_start: Option<bool>,
    pub(super) released: bool,
}

#[derive(Clone, Debug)]
pub(super) struct CircuitInputGeometry {
    pub(super) metrics: LayoutMetrics,
    palette_origin: egui::Pos2,
    palette_rect: egui::Rect,
    palette_layout: PaletteLayout,
}

impl CircuitInputGeometry {
    pub(super) fn new(
        content_rect: egui::Rect,
        screen_rect: egui::Rect,
        layout_qubits: usize,
        min_slots: usize,
        palette: &Palette,
    ) -> Self {
        let palette_layout = palette.layout(screen_rect.width());
        let palette_start_x = palette_start_x(screen_rect.width(), &palette_layout);
        let palette_origin = egui::pos2(
            screen_rect.min.x + palette_start_x,
            screen_rect.min.y + PALETTE_ROW_Y,
        );
        let palette_rect = egui::Rect::from_min_size(
            palette_origin,
            egui::vec2(palette_layout.total_width, palette_layout.total_height),
        );
        let metrics = layout_metrics(content_rect.width(), layout_qubits, min_slots);
        Self {
            metrics,
            palette_origin,
            palette_rect,
            palette_layout,
        }
    }
}

pub(super) struct DragController;

/// Column index the pointer is hovering over for step-preview.
/// Returns `None` when outside the slot row / range.
fn step_at_cursor(cursor: egui::Pos2, metrics: &LayoutMetrics) -> Option<CircuitColumnIndex> {
    if metrics.slot_centers.is_empty() || metrics.line_ys.is_empty() {
        return None;
    }
    let top = metrics.line_ys[0] - STEP_OUTSET_Y;
    let bottom = metrics.line_ys[metrics.line_ys.len() - 1] + STEP_OUTSET_Y;
    if cursor.y < top || cursor.y > bottom {
        return None;
    }
    // The initial step occupies one full slot width up to the first gate's
    // left edge. The column-zero step begins at that edge, not half a slot
    // before its center. Coordinates are circuit-local even in narrow gutters.
    let first_gate_left = metrics.slot_left - GATE_SIZE / 2.0;
    let initial_left = first_gate_left - SLOT_SPACING;
    if cursor.x >= initial_left && cursor.x < first_gate_left {
        return Some(CircuitColumnIndex::INITIAL);
    }
    if cursor.x < first_gate_left || cursor.x > metrics.slot_right + SLOT_SPACING * 0.5 {
        return None;
    }
    let (slot, dist) = nearest_slot_index(cursor.x, &metrics.slot_centers)?;
    if dist <= SLOT_SPACING * 0.5 {
        Some(CircuitColumnIndex::new(slot))
    } else {
        None
    }
}

fn reset_drag_frame_state(app: &mut QniApp) {
    app.drag_repaint_deadline = None;
    app.drag_repaint_pending = false;
}
