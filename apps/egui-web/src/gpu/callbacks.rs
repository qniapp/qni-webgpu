//! `egui_wgpu::CallbackTrait` implementations.
//!
//! Each callback owns one GPU-rendered overlay / pass and pulls resources from
//! the shared `StateVectorResources` slot inside `egui_wgpu::CallbackResources`.
//! Production rendering stays GPU-resident; readback handles exposed here are
//! updated only so test-only wasm APIs can read on demand.

mod bloch_overlay;
mod chance_display;
mod measurement_digit;
mod popup_value;
mod state_vector;

pub(crate) use bloch_overlay::BlochOverlayCallback;
pub(crate) use chance_display::ChanceDisplayCallback;
pub(crate) use measurement_digit::MeasurementDigitCallback;
pub(crate) use popup_value::PopupValueCallback;
pub(crate) use state_vector::StateVectorCallback;
