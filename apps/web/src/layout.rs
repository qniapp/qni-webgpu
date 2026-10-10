//! Circuit and palette geometry facade.
//!
//! Submodules keep the coordinate responsibilities separate: circuit-line /
//! gate-body geometry, qni-style snap target selection, and palette hit testing.

mod geometry;
mod palette;
mod snap;

pub(crate) use geometry::{
    amplitude_cell_index_at, amplitude_grid_dims, amplitude_grid_rect,
    density_matrix_cell_index_at, gate_rect_at_grid, gate_visible_rect, gate_width_cols,
    layout_metrics, nearest_line, nearest_slot_index, LayoutMetrics,
};
pub(crate) use palette::{palette_start_x, Palette, PaletteLayout};
pub(crate) use snap::{nearest_circuit_snap, CircuitSnap};
