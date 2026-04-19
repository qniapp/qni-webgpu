mod gpu;
mod icons;
mod layout;
mod app;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GateKind {
    H,
    Control,
    X,
    Y,
    Z,
    SqrtX,
    S,
    SDagger,
    T,
    TDagger,
    Phase,
    Rx,
    Ry,
    Rz,
    Swap,
}

impl GateKind {
    fn label(self) -> &'static str {
        match self {
            GateKind::H => "H",
            GateKind::Control => "C",
            GateKind::X => "X",
            GateKind::Y => "Y",
            GateKind::Z => "Z",
            GateKind::SqrtX => "√X",
            GateKind::S => "S",
            GateKind::SDagger => "S†",
            GateKind::T => "T",
            GateKind::TDagger => "T†",
            GateKind::Phase => "P",
            GateKind::Rx => "Rx",
            GateKind::Ry => "Ry",
            GateKind::Rz => "Rz",
            GateKind::Swap => "SWAP",
        }
    }
}

const PALETTE_GATES: [GateKind; 15] = [
    GateKind::H,
    GateKind::Control,
    GateKind::X,
    GateKind::Y,
    GateKind::Z,
    GateKind::SqrtX,
    GateKind::S,
    GateKind::SDagger,
    GateKind::T,
    GateKind::TDagger,
    GateKind::Phase,
    GateKind::Rx,
    GateKind::Ry,
    GateKind::Rz,
    GateKind::Swap,
];

#[derive(Clone, Copy, Debug)]
struct GateMatrix {
    m00: [f32; 2],
    m01: [f32; 2],
    m10: [f32; 2],
    m11: [f32; 2],
}

fn gate_matrix(kind: GateKind) -> GateMatrix {
    let inv_sqrt2 = std::f32::consts::FRAC_1_SQRT_2;
    let default_angle = std::f32::consts::FRAC_PI_2;
    let half_angle = default_angle * 0.5;
    let cos_half = half_angle.cos();
    let sin_half = half_angle.sin();
    let exp_i = |angle: f32| [angle.cos(), angle.sin()];
    match kind {
        GateKind::H => GateMatrix {
            m00: [inv_sqrt2, 0.0],
            m01: [inv_sqrt2, 0.0],
            m10: [inv_sqrt2, 0.0],
            m11: [-inv_sqrt2, 0.0],
        },
        GateKind::Control => GateMatrix {
            m00: [1.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [1.0, 0.0],
        },
        GateKind::X => GateMatrix {
            m00: [0.0, 0.0],
            m01: [1.0, 0.0],
            m10: [1.0, 0.0],
            m11: [0.0, 0.0],
        },
        GateKind::Y => GateMatrix {
            m00: [0.0, 0.0],
            m01: [0.0, -1.0],
            m10: [0.0, 1.0],
            m11: [0.0, 0.0],
        },
        GateKind::Z => GateMatrix {
            m00: [1.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [-1.0, 0.0],
        },
        GateKind::SqrtX => GateMatrix {
            m00: [0.5, 0.5],
            m01: [0.5, -0.5],
            m10: [0.5, -0.5],
            m11: [0.5, 0.5],
        },
        GateKind::S => GateMatrix {
            m00: [1.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [0.0, 1.0],
        },
        GateKind::SDagger => GateMatrix {
            m00: [1.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [0.0, -1.0],
        },
        GateKind::T => GateMatrix {
            m00: [1.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [inv_sqrt2, inv_sqrt2],
        },
        GateKind::TDagger => GateMatrix {
            m00: [1.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [inv_sqrt2, -inv_sqrt2],
        },
        GateKind::Phase => GateMatrix {
            m00: [1.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: exp_i(default_angle),
        },
        GateKind::Rx => GateMatrix {
            m00: [cos_half, 0.0],
            m01: [0.0, -sin_half],
            m10: [0.0, -sin_half],
            m11: [cos_half, 0.0],
        },
        GateKind::Ry => GateMatrix {
            m00: [cos_half, 0.0],
            m01: [-sin_half, 0.0],
            m10: [sin_half, 0.0],
            m11: [cos_half, 0.0],
        },
        GateKind::Rz => GateMatrix {
            m00: [cos_half, -sin_half],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [cos_half, sin_half],
        },
        GateKind::Swap => GateMatrix {
            m00: [1.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [1.0, 0.0],
        },
    }
}

fn display_index_to_state_index(mut display_index: usize, qubits: usize) -> usize {
    let mut value = 0usize;
    for _ in 0..qubits {
        value = (value << 1) | (display_index & 1);
        display_index >>= 1;
    }
    value
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GateParams {
    m00: [f32; 2],
    m01: [f32; 2],
    m10: [f32; 2],
    m11: [f32; 2],
    bit: u32,
    state_count: u32,
    control_mask: u32,
    control_value: u32,
}

fn gate_params(kind: GateKind, bit: u32, state_count: u32) -> GateParams {
    let matrix = gate_matrix(kind);
    GateParams {
        m00: matrix.m00,
        m01: matrix.m01,
        m10: matrix.m10,
        m11: matrix.m11,
        bit,
        state_count,
        control_mask: 0,
        control_value: 0,
    }
}

fn gate_params_controlled(
    kind: GateKind,
    bit: u32,
    control_mask: u32,
    control_value: u32,
    state_count: u32,
) -> GateParams {
    let matrix = gate_matrix(kind);
    GateParams {
        m00: matrix.m00,
        m01: matrix.m01,
        m10: matrix.m10,
        m11: matrix.m11,
        bit,
        state_count,
        control_mask,
        control_value,
    }
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

struct Colors {
    background: egui::Color32,
    surface: egui::Color32,
    line: egui::Color32,
    box_fill: egui::Color32,
    box_border: egui::Color32,
    label: egui::Color32,
    text: egui::Color32,
    state_fill: egui::Color32,
    state_outline: egui::Color32,
    state_outline_zero: egui::Color32,
    state_needle: egui::Color32,
}

impl Colors {
    fn new() -> Self {
        Self {
            background: color_rgba(0.976, 0.98, 0.984, 1.0),
            surface: color_rgba(1.0, 1.0, 1.0, 1.0),
            line: color_rgba(0.72, 0.72, 0.72, 1.0),
            box_fill: color_rgba(0.2, 0.62, 0.55, 1.0),
            box_border: color_rgba(0.82, 0.82, 0.82, 1.0),
            label: color_rgba(1.0, 1.0, 1.0, 1.0),
            text: color_rgba(0.45, 0.45, 0.45, 1.0),
            state_fill: color_rgba(0.055, 0.647, 0.914, 1.0), // Tailwind sky-500: rgb(14, 165, 233)
            state_outline: color_rgba(0.0, 0.0, 0.0, 1.0),
            state_outline_zero: color_rgba(0.75, 0.75, 0.75, 1.0),
            state_needle: color_rgba(0.0, 0.0, 0.0, 1.0),
        }
    }
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
