//! Gate icon drawing facade.
//!
//! The implementation is split by responsibility so visual changes stay
//! small: Bloch rendering, gate body fills, SVG glyphs, QFT affordances,
//! and shared viewBox helpers.

mod bloch;
mod gate_body;
mod gate_glyphs;
mod qft;
mod svg;

const VIEWBOX: f32 = 48.0;

pub(crate) use bloch::draw_bloch_vector;
pub(crate) use gate_body::{draw_drag_gate_body, draw_gate_body};
pub(crate) use gate_glyphs::draw_meter_icon;
pub(crate) use qft::draw_qft_resize_handle;
