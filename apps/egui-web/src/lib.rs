mod app;
mod colors;
mod constants;
mod gates;
mod gpu;
mod icons;
mod layout;
mod render;
mod shared;
mod simulation_plan;
mod url_circuit;

use crate::app::QniApp;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn start(canvas_id: &str) -> Result<(), wasm_bindgen::JsValue> {
    let window =
        web_sys::window().ok_or_else(|| wasm_bindgen::JsValue::from_str("window not found"))?;
    let document = window
        .document()
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("document not found"))?;
    let canvas = document
        .get_element_by_id(canvas_id)
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("canvas not found"))?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;

    let web_options = eframe::WebOptions::default();
    eframe::WebRunner::new()
        .start(
            canvas,
            web_options,
            Box::new(|cc| Ok(Box::new(QniApp::new(cc)))),
        )
        .await
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn read_state_vector() -> Result<js_sys::Float32Array, wasm_bindgen::JsValue> {
    gpu::read_state_vector_impl().await
}

/// Test-only on-demand readback for Bloch vectors. Triggers a fresh
/// staging-buffer copy + `map_async` against `bloch_output_buffer` and
/// returns `[gate_id, x, y, z, …]` once the GPU finishes. Production code
/// never calls this — the rendering shaders read the same buffer directly.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn read_bloch_vectors() -> Result<js_sys::Float32Array, wasm_bindgen::JsValue> {
    gpu::read_bloch_vectors_impl().await
}

/// Test-only on-demand readback for measurement outcomes. Returns
/// `[gate_id, outcome, …]` (outcome is `0.0` or `1.0`).
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn read_measurement_outcomes() -> Result<js_sys::Float32Array, wasm_bindgen::JsValue> {
    gpu::read_measurement_outcomes_impl().await
}
