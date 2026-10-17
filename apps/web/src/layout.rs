//! Circuit and palette geometry facade.
//!
//! Submodules keep the coordinate responsibilities separate: circuit-line /
//! gate-body geometry, qni-style snap target selection, and palette hit testing.

mod geometry;
mod orientation;
mod palette;
mod palette_carousel;
mod projection;
mod snap;

pub(crate) use geometry::{
    amplitude_cell_index_at, amplitude_grid_dims, amplitude_grid_rect,
    density_matrix_cell_index_at, gate_rect_at_grid, gate_size, gate_visible_rect, gate_width_cols,
    layout_metrics, layout_metrics_with_gutters, nearest_line, nearest_slot_index, CircuitGutters,
    LayoutMetrics,
};
pub(crate) use orientation::CircuitOrientation;
pub(crate) use palette::{palette_start_x, Palette, PaletteLayout};
pub(crate) use palette_carousel::{CarouselHit, PaletteCarousel};
pub(crate) use projection::CircuitProjection;
pub(crate) use snap::{nearest_circuit_snap, CircuitSnap};
