//! Circuit and palette geometry facade.
//!
//! Submodules keep the coordinate responsibilities separate: circuit-line /
//! gate-body geometry, qni-style snap target selection, and palette hit testing.

mod geometry;
mod palette;
mod snap;

pub(crate) use geometry::{
    amplitude_cell_index_at, amplitude_grid_dims, amplitude_grid_rect,
    density_matrix_cell_index_at, gate_visible_rect, gate_width_cols, layout_metrics, nearest_line,
    nearest_slot_index, LayoutMetrics,
};
pub(crate) use palette::{
    palette_gate_local_pos, palette_hit_test, palette_layout, palette_start_x, PaletteLayout,
};
pub(crate) use snap::{nearest_circuit_snap, CircuitSnap};
