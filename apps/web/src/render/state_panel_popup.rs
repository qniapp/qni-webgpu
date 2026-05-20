//! State panel hover popup and aspect-ratio popover drawing facade.
//!
//! Submodules keep cell-popup layout/paint, aspect popover rows, and tiny qni
//! icons separate while preserving the public render entry points.

mod aspect;
mod cell;
mod icons;

pub(super) use aspect::draw_aspect_popover;
pub(super) use cell::draw_state_cell_popup;
