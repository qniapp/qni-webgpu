//! `egui_wgpu::CallbackTrait` implementations.
//!
//! Each callback owns one GPU-rendered overlay / pass and pulls resources from
//! the shared `StateVectorResources` slot inside `egui_wgpu::CallbackResources`.
//! Production rendering stays GPU-resident; readback handles exposed here are
//! updated only so test-only wasm APIs can read on demand.

mod amplitude_display;
mod amplitude_popup_value;
mod bloch_display_overlay;
mod measurement_digit;
mod popup_value;
mod probability_display;
mod probability_popup_value;
mod state_vector;

pub(crate) use amplitude_display::AmplitudeDisplayCallback;
pub(crate) use amplitude_popup_value::AmplitudePopupValueCallback;
pub(crate) use bloch_display_overlay::BlochOverlayCallback;
pub(crate) use measurement_digit::MeasurementDigitCallback;
pub(crate) use popup_value::PopupValueCallback;
pub(crate) use probability_display::ProbabilityDisplayCallback;
pub(crate) use probability_popup_value::ProbabilityPopupValueCallback;
pub(crate) use state_vector::StateVectorCallback;
