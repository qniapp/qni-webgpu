use crate::gates::GateKind;

pub(crate) const REM: f32 = 32.0;
pub(crate) const STATE_CIRCLE_BOTTOM_MARGIN: f32 = 2.0 * REM;

/// Height of the state panel's drag handle (the zinc-100 strip on top
/// showing "N qubits" / "C × R = N states"). Fixed so the strip text stays
/// readable regardless of the circle grid's size — see
/// `docs/state-panel-handle-mockups.html` (G-2).
pub(crate) const STATE_HANDLE_HEIGHT: f32 = 22.0;

/// Per-qubit-count circle-notation geometry. Matches qni's desktop layout
/// in `packages/elements/src/circle-notation-element.ts`:
///   * `updateDimension()` (cols × rows)
///   * `qubitCircleSizePx` (circle px)
///   * `qubitCircleLineWidth` (gap = stroke)
/// qni uses gap == stroke so the circles read as a tight grid with just
/// the outline showing between them.
#[derive(Clone, Copy, Debug)]
pub(crate) struct StateCircleLayout {
    pub(crate) cols: usize,
    pub(crate) rows: usize,
    pub(crate) size: f32,
    pub(crate) line_width: f32,
}

pub(crate) fn state_circle_layout(qubits: usize) -> StateCircleLayout {
    let q = qubits.clamp(1, 16);
    let (cols, rows, size, line_width) = match q {
        1 => (2, 1, 64.0, 2.0),
        2 => (4, 1, 64.0, 2.0),
        3 => (8, 1, 64.0, 2.0),
        4 => (8, 2, 48.0, 2.0),
        5 => (16, 2, 32.0, 2.0),
        6 => (16, 4, 32.0, 2.0),
        7 => (32, 4, 16.0, 1.0),
        8 => (32, 8, 16.0, 1.0),
        9 => (32, 16, 16.0, 1.0),
        10 => (32, 32, 16.0, 1.0),
        11 => (64, 32, 16.0, 1.0),
        12 => (64, 64, 16.0, 1.0),
        13 => (128, 64, 16.0, 1.0),
        14 => (128, 128, 16.0, 1.0),
        15 => (256, 128, 16.0, 1.0),
        _ => (256, 256, 16.0, 1.0),
    };
    StateCircleLayout {
        cols,
        rows,
        size,
        line_width,
    }
}

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
// qni reference (apps/www/app/views/application/_palette_md.html.erb):
//   space-x-2 / space-y-2 → 0.5rem (8px) inter-gate gap
//   px-4 / py-5           → 1rem (16px) horizontal / 1.25rem (20px) vertical padding
//   rounded-xl            → 0.75rem (12px) border radius
pub(crate) const PALETTE_GAP: f32 = 8.0;
pub(crate) const PALETTE_ROW_GAP: f32 = 8.0;
pub(crate) const PALETTE_PADDING_X: f32 = 16.0;
pub(crate) const PALETTE_PADDING_Y: f32 = 20.0;
pub(crate) const PALETTE_CORNER_RADIUS: u8 = 12;
pub(crate) const PALETTE_ROW_Y: f32 = 2.0 * REM;

// Two-row layout. Row 1 holds the unitary single-qubit gates; row 2 holds the
// special-purpose gates (SWAP/Control/AntiControl/Bloch/|0>/|1>/Measurement/
// Spacer). Indices remain a flat list so callers can look up a gate without
// branching.
pub(crate) const PALETTE_ROW1_COUNT: usize = 13;
pub(crate) const PALETTE_GATES: [GateKind; 21] = [
    // Row 1
    GateKind::H,
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
    // Row 2
    GateKind::Swap,
    GateKind::Control,
    GateKind::AntiControl,
    GateKind::BlochDisplay,
    GateKind::Write0,
    GateKind::Write1,
    GateKind::Measurement,
    GateKind::Spacer,
];
