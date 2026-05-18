//! Circuit and palette geometry facade.
//!
//! Submodules keep the coordinate responsibilities separate: circuit-line /
//! resizable-span geometry, qni-style snap target selection, and palette hit testing.

mod geometry;
mod palette;
mod snap;

pub(crate) use geometry::{
    gate_visible_rect, layout_metrics, nearest_line, nearest_slot_index,
    span_resize_handle_edge_at, span_resize_handle_rect, LayoutMetrics,
};
pub(crate) use palette::{
    palette_gate_local_pos, palette_hit_test, palette_layout, palette_start_x, PaletteLayout,
};
pub(crate) use snap::{nearest_circuit_snap, CircuitSnap};
