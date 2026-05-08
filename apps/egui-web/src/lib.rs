mod app;
mod bloch;
mod colors;
mod constants;
mod gates;
mod gpu;
mod icons;
mod layout;
mod render;
mod shared;

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

/// Returns the most recent Bloch readback as a flat `Float32Array` laid out
/// as `[gate_id, x, y, z, gate_id, x, y, z, …]`. Empty when there are no
/// `BlochDisplay` gates placed (or while a readback is in flight). Used by
/// the test harness to assert per-qubit Bloch vectors without poking at
/// individual canvas pixels.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn read_bloch_vectors() -> js_sys::Float32Array {
    app::read_bloch_vectors_snapshot()
}

/// Returns the most recent measurement readback as a flat `Float32Array`
/// laid out as `[gate_id, outcome, gate_id, outcome, …]`. `outcome` is 0 or
/// 1 (still encoded as f32 for transport). Empty when there are no
/// `Measurement` gates placed (or while a readback is in flight).
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn read_measurement_outcomes() -> js_sys::Float32Array {
    app::read_measurement_outcomes_snapshot()
}
