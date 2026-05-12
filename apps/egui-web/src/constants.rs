pub(crate) const REM: f32 = 32.0;
pub(crate) const STATE_CIRCLE_BOTTOM_MARGIN: f32 = 2.0 * REM;

/// Height of the state panel's header strip (the blue-600 bar at the top
/// showing "N qubits" / "C × R = N states", and that hosts the top resize
/// handles). Tailwind h-8 (32px), giving text-sm (14px / 20px LH) ~6 px
/// of breathing room above and below — a comfortable "header" feel
/// without crowding the strip's 10×10 corner resize handles.
pub(crate) const STATE_HANDLE_HEIGHT: f32 = 32.0;

/// Initial viewport size (the area below the header strip where circles
/// render). The user can resize the panel by dragging the corner L
/// handles, clamped to `STATE_VIEWPORT_{MIN,MAX}_{WIDTH,HEIGHT}`.
pub(crate) const STATE_VIEWPORT_DEFAULT_WIDTH: f32 = 560.0;
pub(crate) const STATE_VIEWPORT_DEFAULT_HEIGHT: f32 = 160.0;
pub(crate) const STATE_VIEWPORT_MIN_WIDTH: f32 = 280.0;
pub(crate) const STATE_VIEWPORT_MIN_HEIGHT: f32 = 80.0;
pub(crate) const STATE_VIEWPORT_MAX_WIDTH: f32 = 1200.0;
pub(crate) const STATE_VIEWPORT_MAX_HEIGHT: f32 = 600.0;

/// Panel rounded-corner radius (px). The top resize handles' outer arcs
/// curve along this radius minus the handle padding so they sit flush
/// against the panel's rounded inner edge. Mirrored by `state_corner` in
/// `render.rs`.
pub(crate) const STATE_PANEL_CORNER_RADIUS: f32 = 14.0;

/// Geometry of the 4 corner resize handles (G 案 / 内側配置).
/// The handle is a 90° arc segment of a circle <strong>concentric with
/// the panel's rounded corner</strong>: the same centre, radius =
/// (panel R − PAD). This makes the handle literally trace the panel's
/// rounded inner edge, offset inward by PAD.
///   * PAD: inward offset from the panel's outer rounded edge, 3 px.
///   * STROKE: handle line thickness, 2 px.
///   * HIT_PAD: extra padding on the hit rect for forgiving grabs.
pub(crate) const STATE_RESIZE_HANDLE_PAD: f32 = 3.0;
pub(crate) const STATE_RESIZE_HANDLE_STROKE: f32 = 2.0;
pub(crate) const STATE_RESIZE_HIT_PAD: f32 = 2.0;

/// Zoom factor range for the state panel circle grid (Ctrl+wheel). Below
/// 0.5 the cells become unreadable noise; above 4.0 the panel is mostly
/// the inside of one circle.
pub(crate) const STATE_GRID_ZOOM_MIN: f32 = 0.5;
pub(crate) const STATE_GRID_ZOOM_MAX: f32 = 4.0;

/// QFT / QFT† resizable-span gate geometry. Matches qni's
/// `--qni-component-resize-handle-width / -height` (= operation-base /
/// operation-base-¾) — a square-ish purple button below the gate body
/// the user grabs to change span.
///   * MAX_SPAN: clamp the dragged span to this many qubit wires.
///   * RESIZE_HANDLE_WIDTH: full gate width (= GATE_SIZE) so the
///     handle sits flush under the body.
///   * RESIZE_HANDLE_HEIGHT: 0.75 * GATE_SIZE (qni's `-¾` size).
pub(crate) const QFT_MAX_SPAN: usize = 16;
pub(crate) const QFT_RESIZE_HANDLE_WIDTH: f32 = GATE_SIZE;
pub(crate) const QFT_RESIZE_HANDLE_HEIGHT: f32 = GATE_SIZE * 0.75;

/// How much accumulated wheel scroll delta = one aspect-step on the
/// dims text. A typical OS wheel notch is roughly 50–100 px of smooth
/// scroll delta, so 180 makes ~2–4 notches per step — heavy enough
/// that a single flick lands on the next aspect without over-shooting,
/// light enough that long swipes still get through.
pub(crate) const ASPECT_WHEEL_PER_STEP: f32 = 180.0;

/// Per-qubit-count circle-notation geometry. Cell size + line width
/// follow qni's desktop layout (`circle-notation-element.ts:qubitCircleSizePx`
/// / `qubitCircleLineWidth`). The (cols, rows) split is parameterised by
/// `aspect_index = log2(cols)`, so the user can change the layout's
/// aspect at runtime without touching cell size (via the aspect popover
/// on the dims text in the state panel header). qni uses gap == stroke
/// so the circles read as a tight grid with just the outline showing
/// between them.
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

/// Default aspect index per qubit count. Mirrors qni's hard-coded
/// (cols, rows) layout — used on app start and when the user hasn't yet
/// customised the aspect.
pub(crate) fn state_circle_default_aspect_index(qubits: usize) -> usize {
    let q = qubits.clamp(1, 16);
    match q {
        1 => 1,       // 2 × 1
        2 => 2,       // 4 × 1
        3 => 3,       // 8 × 1
        4 => 3,       // 8 × 2
        5 => 4,       // 16 × 2
        6 => 4,       // 16 × 4
        7..=10 => 5,  // 32 × {4..32}
        11 | 12 => 6, // 64 × {32, 64}
        13 | 14 => 7, // 128 × {64, 128}
        _ => 8,       // 256 × {128, 256}
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
