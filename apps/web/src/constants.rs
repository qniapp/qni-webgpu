pub(crate) const REM: f32 = 32.0;
pub(crate) const STATE_CIRCLE_BOTTOM_MARGIN: f32 = 2.0 * REM;

pub(crate) const STATE_HANDLE_HEIGHT: f32 = crate::tailwind::SPACING_8;

pub(crate) const STATE_VIEWPORT_DEFAULT_WIDTH: f32 = 560.0;
pub(crate) const STATE_VIEWPORT_DEFAULT_HEIGHT: f32 = 160.0;
pub(crate) const STATE_VIEWPORT_MIN_WIDTH: f32 = 280.0;
pub(crate) const STATE_VIEWPORT_MIN_HEIGHT: f32 = 80.0;
pub(crate) const STATE_VIEWPORT_MAX_WIDTH: f32 = 1200.0;
pub(crate) const STATE_VIEWPORT_MAX_HEIGHT: f32 = 600.0;
pub(crate) const EMBED_STATE_PANEL_CIRCUIT_GAP: f32 = crate::tailwind::SPACING_4;
pub(crate) const EMBED_STATE_PANEL_TIGHT_BOTTOM_MARGIN: f32 = crate::tailwind::SPACING_4;

pub(crate) const STATE_PANEL_CORNER_RADIUS: f32 = 14.0;

pub(crate) const STATE_RESIZE_HANDLE_PAD: f32 = 3.0;
pub(crate) const STATE_RESIZE_HANDLE_STROKE: f32 = 2.0;
pub(crate) const STATE_RESIZE_HIT_PAD: f32 = 2.0;

pub(crate) const STATE_GRID_CIRCLE_SIZE_MIN: f32 = 1.0;
pub(crate) const STATE_GRID_CIRCLE_SIZE_MAX: f32 = 256.0;

pub(crate) fn state_grid_zoom_limits(natural_circle_size: f32) -> (f32, f32) {
    let natural = natural_circle_size.max(f32::EPSILON);
    (
        STATE_GRID_CIRCLE_SIZE_MIN / natural,
        STATE_GRID_CIRCLE_SIZE_MAX / natural,
    )
}

pub(crate) const ASPECT_WHEEL_PER_STEP: f32 = 180.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct StateCircleLayout {
    pub(crate) cols: usize,
    pub(crate) rows: usize,
    pub(crate) size: f32,
    pub(crate) line_width: f32,
}

pub(crate) fn state_circle_layout(qubits: usize, aspect_index: usize) -> StateCircleLayout {
    let q = qubits.clamp(1, 16);
    let aspect = aspect_index.min(q);
    let cols = 1usize << aspect;
    let rows = 1usize << (q - aspect);
    let (size, line_width) = match q {
        1..=3 => (64.0, 2.0),
        4 => (48.0, 2.0),
        5..=6 => (32.0, 2.0),
        _ => (16.0, 1.0),
    };
    StateCircleLayout {
        cols,
        rows,
        size,
        line_width,
    }
}

pub(crate) fn state_circle_default_aspect_index(qubits: usize) -> usize {
    let q = qubits.clamp(1, 16);
    match q {
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 3,
        5 => 4,
        6 => 4,
        7..=10 => 5,
        11 | 12 => 6,
        13 | 14 => 7,
        _ => 8,
    }
}

pub(crate) const MIN_QUBITS: usize = 2;
pub(crate) const LOCAL_MAX_QUBITS: usize = 16;
pub(crate) const GPU_MAX_QUBITS: usize = 32;
pub(crate) const MAX_QUBITS: usize = LOCAL_MAX_QUBITS;
pub(crate) const MAX_STATE_COUNT: usize = 1 << MAX_QUBITS;

pub(crate) const PALETTE_CIRCUIT_GAP: f32 = crate::tailwind::SPACING_12;

pub(crate) const LINE_Y: f32 = PALETTE_ROW_Y
    + (PALETTE_SIZE * 2.0 + PALETTE_ROW_GAP)
    + PALETTE_PADDING_Y
    + PALETTE_CIRCUIT_GAP
    + GATE_SIZE / 2.0;
// 56px. With GATE_SIZE = 40 this preserves the previous
// 16 px visual gap between gates on neighbouring wires (48 - 32 = 56 - 40).
pub(crate) const LINE_GAP: f32 = crate::tailwind::SPACING_14;
pub(crate) const STEP_OUTSET_Y: f32 = GATE_SIZE / 2.0 + crate::tailwind::SPACING_8;
pub(crate) const CIRCUIT_PADDING: f32 = crate::tailwind::SPACING_16;
pub(crate) const QUBIT_LABEL_WIDTH: f32 = 3.0 * crate::tailwind::TEXT_SM;
pub(crate) const QUBIT_LABEL_GAP: f32 = 0.5 * REM;
pub(crate) const LINE_LEFT_OFFSET: f32 = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP;
pub(crate) const LINE_RIGHT_OFFSET: f32 = CIRCUIT_PADDING;
/// Narrower circuit areas use compact gutters so a phone-width embed (a 354px
/// canvas on a 390px tutorial page) still shows five columns.
pub(crate) const COMPACT_CIRCUIT_MAX_WIDTH: f32 = crate::tailwind::SCREEN_SM;
pub(crate) const COMPACT_LINE_LEFT_OFFSET: f32 = crate::tailwind::SPACING_10;
pub(crate) const COMPACT_QUBIT_LABEL_GAP: f32 = crate::tailwind::SPACING_2;
pub(crate) const COMPACT_CIRCUIT_PADDING: f32 = crate::tailwind::SPACING_4;
pub(crate) const VERTICAL_LINE_TOP_OFFSET: f32 = crate::tailwind::SPACING_8;
pub(crate) const VERTICAL_WIRE0_RIGHT_INSET: f32 = STEP_OUTSET_Y + VERTICAL_SCREEN_INSET;
pub(crate) const VERTICAL_SCREEN_INSET: f32 = crate::tailwind::SPACING_2;
pub(crate) const VERTICAL_CIRCUIT_GAP: f32 = crate::tailwind::SPACING_4;
pub(crate) const TOOLBAR_HEIGHT: f32 =
    crate::tailwind::SPACING_8 + 2.0 * crate::tailwind::SPACING_1_5;

pub(crate) const CAROUSEL_GATES_PER_PAGE: usize = 4;
pub(crate) const CAROUSEL_GATE_SIZE: f32 = crate::tailwind::SPACING_12;
pub(crate) const CAROUSEL_GATE_GAP: f32 = crate::tailwind::SPACING_4;
pub(crate) const CAROUSEL_ARROW_SIZE: f32 = crate::tailwind::SPACING_12;
pub(crate) const CAROUSEL_ARROW_INSET: f32 = crate::tailwind::SPACING_1;
pub(crate) const CAROUSEL_DOT_SIZE: f32 = crate::tailwind::SPACING_2;
pub(crate) const CAROUSEL_DOT_GAP: f32 = crate::tailwind::SPACING_2;
pub(crate) const CAROUSEL_DOT_TOP_GAP: f32 = crate::tailwind::SPACING_3;
pub(crate) const CAROUSEL_PADDING_TOP: f32 = crate::tailwind::SPACING_4;
pub(crate) const CAROUSEL_PADDING_BOTTOM: f32 = crate::tailwind::SPACING_3;

// qni circuit-block geometry, from the `min-width: 768px` rules in
// `packages/elements/src/circuit-block-element.ts` (qniapp/qni@acf87bf):
// `#body { margin: 1.5rem 0; padding: 2rem 0; border-width: 2px 0 }`.
// The body wraps the step-preview bars (half a wire gap above the first
// wire / below the last) and each label sits in the margin outside a rule.
// 32px between a rule and the step-preview bar end.
pub(crate) const CIRCUIT_BLOCK_PADDING_Y: f32 = crate::tailwind::SPACING_8;
// 2 px top / bottom rules, as qni's `border-top-width: 2px`.
pub(crate) const CIRCUIT_BLOCK_BORDER_WIDTH: f32 = 2.0;
// 24px label band outside each rule.
pub(crate) const CIRCUIT_BLOCK_MARGIN_Y: f32 = crate::tailwind::SPACING_6;
// 16px monospace label, as qni's block comment.
pub(crate) const CIRCUIT_BLOCK_LABEL_FONT_SIZE: f32 = crate::tailwind::TEXT_BASE;
// 28px label line box, the line height qni's label inherits.
// It is anchored to the outer edge of the margin, so it overlaps the
// rule by 28 - 24 = 4 px.
pub(crate) const CIRCUIT_BLOCK_LABEL_LINE_HEIGHT: f32 = crate::tailwind::LEADING_7;
pub(crate) const CIRCUIT_BLOCK_OUTSET_Y: f32 =
    CIRCUIT_BLOCK_PADDING_Y + CIRCUIT_BLOCK_BORDER_WIDTH + CIRCUIT_BLOCK_MARGIN_Y;

pub(crate) const GATE_SIZE: f32 = crate::tailwind::SPACING_10;
// 56px. Kept equal to LINE_GAP for a square visual rhythm.
pub(crate) const SLOT_SPACING: f32 = crate::tailwind::SPACING_14;
// 40px, matching the 40 px gate body for Qni-style sticky snap.
pub(crate) const SNAP_DISTANCE: f32 = crate::tailwind::SPACING_10;
pub(crate) const DRAG_REPAINT_BASE_SECS: f64 = 0.01;
pub(crate) const DRAG_REPAINT_MIN_SECS: f64 = 0.004;
pub(crate) const DRAG_REPAINT_MAX_SECS: f64 = 1.0 / 30.0;
pub(crate) const DRAG_REPAINT_PUMP_FACTOR: f64 = 0.1;
pub(crate) const PALETTE_SIZE: f32 = GATE_SIZE;
pub(crate) const PALETTE_GAP: f32 = crate::tailwind::SPACING_2;
pub(crate) const PALETTE_ROW_GAP: f32 = crate::tailwind::SPACING_2;
pub(crate) const PALETTE_SECTION_GAP: f32 = crate::tailwind::SPACING_2;
pub(crate) const SECTION_DIVIDER_WIDTH: f32 = 2.0;
pub(crate) const PALETTE_SEPARATOR_WIDTH: f32 = SECTION_DIVIDER_WIDTH;
pub(crate) const PALETTE_DISPLAY_COLUMNS: usize = 4;
pub(crate) const PALETTE_DISPLAY_ROWS: usize = 2;
pub(crate) const PALETTE_PADDING_X: f32 = crate::tailwind::SPACING_4;
pub(crate) const PALETTE_PADDING_Y: f32 = crate::tailwind::SPACING_5;
pub(crate) const PALETTE_MARGIN_X: f32 = crate::tailwind::SPACING_4;
pub(crate) const PALETTE_CORNER_RADIUS: u8 = crate::tailwind::ROUNDED_XL;
pub(crate) const PALETTE_ROW_Y: f32 = 2.5 * REM;
