//! WGSL shader source strings split by rendering / compute responsibility.
//!
//! Keep the public surface as constants so resource builders can import
//! shader sources without knowing which file owns each WGSL program.

mod bloch;
mod chance;
mod digit;
mod measure;
mod popup_value;
mod state;

pub(super) use bloch::{BLOCH_OVERLAY_SHADER, BLOCH_REDUCE_SHADER};
pub(super) use chance::{CHANCE_REDUCE_SHADER, CHANCE_RENDER_SHADER};
pub(super) use digit::MEASUREMENT_DIGIT_SHADER;
pub(super) use measure::{MEASURE_COLLAPSE_SHADER, MEASURE_REDUCE_SHADER};
pub(super) use popup_value::POPUP_VALUE_SHADER;
pub(super) use state::{STATE_COMPUTE_SHADER, STATE_RENDER_SHADER};
