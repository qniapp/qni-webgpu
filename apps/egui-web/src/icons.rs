//! Gate icon drawing facade.
//!
//! The implementation is split by responsibility so visual changes stay
//! small: Bloch rendering, gate body fills, SVG glyphs, resizable-span
//! affordances, and shared viewBox helpers.

mod bloch;
mod gate_body;
mod gate_glyphs;
mod sdf_icon;
mod span_resize;
mod svg;
mod svg_icon;

const VIEWBOX: f32 = 48.0;

pub(crate) use bloch::draw_bloch_vector;
pub(crate) use gate_body::{draw_drag_gate_body, draw_gate_body};
pub(crate) use gate_glyphs::draw_meter_icon;
pub(crate) use sdf_icon::set_target_format as set_sdf_target_format;
pub(crate) use span_resize::{draw_chance_resize_handle, draw_qft_resize_handle};
