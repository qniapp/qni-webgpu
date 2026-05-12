//! Circuit and palette geometry facade.
//!
//! Submodules keep the coordinate responsibilities separate: circuit-line / QFT
//! geometry, qni-style snap target selection, and palette hit testing.

mod geometry;
mod palette;
mod snap;

pub(crate) use geometry::{
    gate_visible_rect, layout_metrics, nearest_line, nearest_slot_index, qft_resize_handle_rect,
    LayoutMetrics,
};
pub(crate) use palette::{palette_gate_local_pos, palette_hit_test, palette_layout, PaletteLayout};
pub(crate) use snap::{nearest_circuit_snap, CircuitSnap};
