//! Gate icon drawing facade.
//!
//! The implementation is split by responsibility so visual changes stay
//! small: Bloch rendering, gate body fills, SVG glyphs, resizable-span
//! affordances, and shared viewBox helpers.

mod bloch_display;
mod gate_body;
mod gate_glyphs;
mod sdf_icon;
mod span_resize;
mod svg;
mod svg_icon;

const VIEWBOX: f32 = 48.0;

pub(crate) use bloch_display::draw_bloch_vector;
pub(crate) use gate_body::{draw_drag_gate_body, draw_gate_body};
pub(crate) use gate_glyphs::draw_meter_icon;
pub(crate) use sdf_icon::set_target_format as set_sdf_target_format;
pub(crate) use span_resize::draw_span_resize_handle;
pub(crate) use svg_icon::{digit_sdf_rle, DIGIT_SDF_PX_RANGE, DIGIT_SDF_SIZE};
