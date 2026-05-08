use crate::gates::GateKind;

pub(crate) const REM: f32 = 32.0;
pub(crate) const STATE_CIRCLE_SIZE: f32 = 1.25 * REM;
pub(crate) const STATE_CIRCLE_GAP: f32 = 0.5 * REM;
pub(crate) const STATE_CIRCLE_BOTTOM_MARGIN: f32 = 2.0 * REM;
pub(crate) const STATE_CIRCLE_STROKE: f32 = 2.0;

pub(crate) const MIN_QUBITS: usize = 2;
pub(crate) const MAX_QUBITS: usize = 16;
pub(crate) const MAX_STATE_COUNT: usize = 1 << MAX_QUBITS;

pub(crate) const LINE_Y: f32 = 6.5 * REM;
pub(crate) const LINE_GAP: f32 = 1.5 * REM;
pub(crate) const CIRCUIT_PADDING: f32 = 2.0 * REM; // Same as PALETTE_ROW_Y for visual consistency
pub(crate) const QUBIT_LABEL_WIDTH: f32 = 3.0 * 14.0; // "qN:" at font size 14
pub(crate) const QUBIT_LABEL_GAP: f32 = 0.5 * REM; // Gap between label and line (0.5rem)
pub(crate) const LINE_LEFT_OFFSET: f32 = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP;
pub(crate) const LINE_RIGHT_OFFSET: f32 = CIRCUIT_PADDING;

pub(crate) const GATE_SIZE: f32 = 1.0 * REM;
pub(crate) const SLOT_SPACING: f32 = GATE_SIZE * 1.5;
pub(crate) const SNAP_DISTANCE: f32 = 0.5625 * REM;
pub(crate) const DRAG_REPAINT_BASE_SECS: f64 = 0.01;
pub(crate) const DRAG_REPAINT_MIN_SECS: f64 = 0.004;
pub(crate) const DRAG_REPAINT_MAX_SECS: f64 = 1.0 / 30.0;
pub(crate) const DRAG_REPAINT_PUMP_FACTOR: f64 = 0.1;
pub(crate) const PALETTE_SIZE: f32 = GATE_SIZE;
pub(crate) const PALETTE_GAP: f32 = 0.5 * REM;
pub(crate) const PALETTE_ROW_Y: f32 = 2.0 * REM;

pub(crate) const PALETTE_GATES: [GateKind; 16] = [
    GateKind::H,
    GateKind::Control,
    GateKind::AntiControl,
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
