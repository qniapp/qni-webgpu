//! Render module — entry point. The implementation is split across
//! three submodules by responsibility:
//!   * `circuit`            — qubit lines, gates, palette, drag preview
//!   * `state_panel_layout` — geometry & hit rects (no painter)
//!   * `state_panel_draw`   — painter-side drawing for the state panel
//!
//! Each submodule extends `impl QniApp` with the methods it owns.
//! Nothing else lives here.

mod circuit;
mod state_panel_layout;
mod state_panel_draw;

pub(crate) use state_panel_layout::StatePanelLayout;
