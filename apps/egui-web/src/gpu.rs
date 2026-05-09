//! GPU module — WebGPU pipelines, buffers, and egui callbacks.
//!
//! Layered structure:
//! * Layer 0 (leaves): `shaders`, `params`, `digit_atlas`
//! * Layer 1: `resources` — `StateVectorResources` + its giant `new`
//! * Layer 2: `callbacks` — `egui_wgpu::CallbackTrait` impls
//!           `readback` — test-only `#[wasm_bindgen]` async APIs
//!
//! This file re-exports the public API external callers (`lib.rs`,
//! `render.rs`) depend on so the existing `crate::gpu::Foo` paths keep
//! working.

mod callbacks;
mod digit_atlas;
mod params;
mod readback;
mod resources;
mod shaders;

pub(crate) use callbacks::{BlochOverlayCallback, MeasurementDigitCallback, StateVectorCallback};
pub(crate) use params::{
    BlochOverlayInstance, MeasurementDigitInstance, RenderColors, StateInstance,
};
#[cfg(target_arch = "wasm32")]
pub(crate) use readback::{
    read_bloch_vectors_impl, read_measurement_outcomes_impl, read_state_vector_impl,
};
