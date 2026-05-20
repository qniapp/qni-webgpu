//! GPU module — WebGPU pipelines, buffers, and egui callbacks.
//!
//! Layered structure:
//! * Layer 0 (leaves): `shaders`, `params`, `digit_atlas`, `popup_glyph_atlas`
//! * Layer 1: `resources` — `StateVectorResources` aggregator + the
//!   per-subsystem submodules (`common`, `state`, `bloch`, `measure`,
//!   `digit`, `popup_value`) under `resources/`
//! * Layer 2: `callbacks` — `egui_wgpu::CallbackTrait` impls;
//!   `readback` — test-only `#[wasm_bindgen]` async APIs
//!
//! This file re-exports the public API external callers (`lib.rs`,
//! `render.rs`) depend on so the existing `crate::gpu::Foo` paths keep
//! working.

mod callbacks;
mod digit_atlas;
mod params;
mod popup_glyph_atlas;
mod readback;
mod recompute;
mod resources;
mod shaders;

pub(crate) use callbacks::{
    AmplitudeDisplayCallback, AmplitudePopupValueCallback, BlochOverlayCallback,
    MeasurementDigitCallback, PopupValueCallback, ProbabilityDisplayCallback,
    ProbabilityPopupValueCallback, StateVectorCallback,
};
pub(crate) use params::{
    AmplitudeInstance, BlochOverlayInstance, MeasurementDigitInstance, ProbabilityInstance,
    RenderColors, RenderParams, MAX_AMPLITUDE_SLOTS, MAX_BLOCH_SLOTS, MAX_MEASUREMENT_SLOTS,
    MAX_OPS_PER_RECOMPUTE, MAX_PROBABILITY_SLOTS, MAX_STEP_SNAPSHOT_SLOTS,
};
pub(crate) use popup_glyph_atlas::{POPUP_GLYPH_CELL_H, POPUP_GLYPH_CELL_W};
#[cfg(target_arch = "wasm32")]
pub(crate) use readback::{
    read_amplitude_cell_impl, read_bloch_vectors_impl, read_measurement_outcomes_impl,
    read_probability_distributions_impl, read_state_vector_impl,
};
