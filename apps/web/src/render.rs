//! Render module — entry point. The implementation is split across
//! three submodules by responsibility:
//!   * `circuit`            — qubit lines, gates, palette, drag preview
//!   * `state_panel_layout` — geometry & hit rects (no painter)
//!   * `state_panel_draw`   — painter-side drawing for the state panel
//!
//! Each submodule extends `impl QniApp` with the methods it owns.
//! Nothing else lives here.

mod amplitude_circle_popover;
mod angle_input;
mod circuit;
mod circuit_block_editor;
mod circuit_blocks;
mod circuit_connectors;
mod circuit_gates;
mod circuit_palette;
mod circuit_picker;
mod gpu_status_pill;
mod hover_frame;
mod popover;
pub(crate) mod restart_button;
mod state_panel_chrome;
mod state_panel_draw;
mod state_panel_layout;
mod state_panel_popup;
pub(crate) mod toolbar;
mod toolbar_chrome;

pub(crate) use state_panel_layout::{vertical_default_aspect_index, StatePanelLayout};

pub(crate) use circuit_blocks::{
    circuit_block_boundary_x, circuit_block_label_width, circuit_block_rect,
    CIRCUIT_BLOCK_LABEL_GAP_X,
};
pub(crate) use circuit_gates::CircuitGpuBatch;
