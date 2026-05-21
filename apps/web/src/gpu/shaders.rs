//! WGSL shader source strings split by rendering / compute responsibility.
//!
//! Keep the public surface as constants so resource builders can import
//! shader sources without knowing which file owns each WGSL program.

mod amplitude;
mod amplitude_popup_value;
mod bloch_display;
mod digit;
mod measure;
mod popup_value;
mod probability;
mod probability_popup_value;
mod state;

pub(super) use amplitude::{AMPLITUDE_CAPTURE_SHADER, AMPLITUDE_RENDER_SHADER};
pub(super) use amplitude_popup_value::AMPLITUDE_POPUP_VALUE_SHADER;
pub(super) use bloch_display::{BLOCH_OVERLAY_SHADER, BLOCH_REDUCE_SHADER};
pub(super) use digit::MEASUREMENT_DIGIT_SHADER;
pub(super) use measure::{MEASURE_COLLAPSE_SHADER, MEASURE_REDUCE_SHADER};
pub(super) use popup_value::POPUP_VALUE_SHADER;
pub(super) use probability::{
    PROBABILITY_AGGREGATE_SHADER, PROBABILITY_REDUCE_SHADER, PROBABILITY_RENDER_SHADER,
};
pub(super) use probability_popup_value::PROBABILITY_POPUP_VALUE_SHADER;
pub(super) use state::{STATE_COMPUTE_SHADER, STATE_RENDER_SHADER};
