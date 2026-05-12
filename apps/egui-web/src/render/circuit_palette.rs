//! Palette, palette tooltip, and drag-preview drawing facade.
//!
//! Submodules keep palette chrome, tooltip diagram, and in-flight drag preview
//! separate while preserving the `QniApp` drawing methods used by update flow.

mod drag_preview;
mod panel;
mod tooltip;
