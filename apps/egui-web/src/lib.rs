mod app;
mod colors;
mod gates;
mod gpu;
mod icons;
mod layout;
mod render;

use eframe::egui;

use crate::app::QniApp;

const REM: f32 = 32.0;
const STATE_CIRCLE_SIZE: f32 = 1.25 * REM;
const STATE_CIRCLE_GAP: f32 = 0.5 * REM;
const STATE_CIRCLE_BOTTOM_MARGIN: f32 = 2.0 * REM;
const STATE_CIRCLE_STROKE: f32 = 2.0;

const MIN_QUBITS: usize = 2;
const MAX_QUBITS: usize = 16;
const MAX_STATE_COUNT: usize = 1 << MAX_QUBITS;

const LINE_Y: f32 = 6.5 * REM;
const LINE_GAP: f32 = 1.5 * REM;
const CIRCUIT_PADDING: f32 = 2.0 * REM; // Same as PALETTE_ROW_Y for visual consistency
const QUBIT_LABEL_WIDTH: f32 = 3.0 * 14.0; // "qN:" at font size 14
const QUBIT_LABEL_GAP: f32 = 0.5 * REM; // Gap between label and line (0.5rem)
const LINE_LEFT_OFFSET: f32 = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP;
const LINE_RIGHT_OFFSET: f32 = CIRCUIT_PADDING;

const GATE_SIZE: f32 = 1.0 * REM;
const SLOT_SPACING: f32 = GATE_SIZE * 1.5;
const SNAP_DISTANCE: f32 = 0.5625 * REM;
const DRAG_REPAINT_BASE_SECS: f64 = 0.01;
const DRAG_REPAINT_MIN_SECS: f64 = 0.004;
const DRAG_REPAINT_MAX_SECS: f64 = 1.0 / 30.0;
const DRAG_REPAINT_PUMP_FACTOR: f64 = 0.1;
const PALETTE_SIZE: f32 = GATE_SIZE;
const PALETTE_GAP: f32 = 0.5 * REM;
const PALETTE_ROW_Y: f32 = 2.0 * REM;

#[cfg(target_arch = "wasm32")]
fn now_seconds() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map(|performance| performance.now() / 1000.0)
        .unwrap_or(0.0)
}

#[cfg(not(target_arch = "wasm32"))]
fn now_seconds() -> f64 {
    use std::sync::OnceLock;
    use std::time::Instant;

    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

const PALETTE_GATES: [gates::GateKind; 15] = [
    gates::GateKind::H,
    gates::GateKind::Control,
    gates::GateKind::X,
    gates::GateKind::Y,
    gates::GateKind::Z,
    gates::GateKind::SqrtX,
    gates::GateKind::S,
    gates::GateKind::SDagger,
    gates::GateKind::T,
    gates::GateKind::TDagger,
    gates::GateKind::Phase,
    gates::GateKind::Rx,
    gates::GateKind::Ry,
    gates::GateKind::Rz,
    gates::GateKind::Swap,
];

fn display_index_to_state_index(mut display_index: usize, qubits: usize) -> usize {
    let mut value = 0usize;
    for _ in 0..qubits {
        value = (value << 1) | (display_index & 1);
        display_index >>= 1;
    }
    value
}

fn amplitude_qubits(len: usize) -> usize {
    let mut qubits = 0;
    let mut size = 1usize;
    if len == 0 {
        return 1;
    }
    while size < len {
        size <<= 1;
        qubits += 1;
    }
    qubits.max(1)
}

fn color_rgba(r: f32, g: f32, b: f32, a: f32) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
        (a * 255.0).round() as u8,
    )
}

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
