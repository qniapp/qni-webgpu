use eframe::egui;

use crate::app::PlacedGate;
use crate::constants::{
    GATE_SIZE, LINE_GAP, LINE_LEFT_OFFSET, LINE_RIGHT_OFFSET, LINE_Y, PALETTE_GAP, PALETTE_ROW_GAP,
    PALETTE_SIZE, QFT_RESIZE_HANDLE_HEIGHT, QFT_RESIZE_HANDLE_WIDTH, SLOT_SPACING,
};
use crate::gates::{PALETTE_GATES, PALETTE_ROW1_COUNT};

/// Visible rect of a placed gate, accounting for the multi-qubit
/// `span` of QFT-family gates. Single-qubit gates get `GATE_SIZE` ×
/// `GATE_SIZE`; QFT extends downward to cover all wires in its span.
/// `origin` is the top-left of the gate body (= rect.min + gate.pos
/// in the circuit's local coordinate space).
pub(crate) fn gate_visible_rect(gate: &PlacedGate, origin: egui::Pos2) -> egui::Rect {
    let height = if gate.kind.is_resizable_span() {
        let span = gate.span.max(1);
        (span - 1) as f32 * LINE_GAP + GATE_SIZE
    } else {
        GATE_SIZE
    };
    egui::Rect::from_min_size(origin, egui::vec2(GATE_SIZE, height))
}

/// QFT resize-handle bounding box — a square-ish purple button below
/// the gate body, centred horizontally and offset slightly past the
/// bottom edge so it visually reads as a separate affordance. Matches
/// qni's `--qni-component-resize-handle-{width,height}` (= GATE_SIZE
/// × 0.75·GATE_SIZE).
pub(crate) fn qft_resize_handle_rect(gate_rect: egui::Rect) -> egui::Rect {
    let cx = gate_rect.center().x;
    let bottom = gate_rect.max.y;
    let half_w = QFT_RESIZE_HANDLE_WIDTH * 0.5;
    // Small overlap so the handle visually anchors to the body but
    // mostly sits below it (matches qni's "overlap" margin pattern).
    let top = bottom - QFT_RESIZE_HANDLE_HEIGHT * 0.25;
    egui::Rect::from_min_max(
        egui::pos2(cx - half_w, top),
        egui::pos2(cx + half_w, top + QFT_RESIZE_HANDLE_HEIGHT),
    )
}

#[derive(Clone, Debug)]
pub(crate) struct LayoutMetrics {
    pub(crate) line_left: f32,
    pub(crate) line_right: f32,
    pub(crate) line_ys: Vec<f32>,
    pub(crate) slot_left: f32,
    pub(crate) slot_right: f32,
    pub(crate) slot_centers: Vec<f32>,
}

/// Compute layout metrics for the circuit area.
///
/// `min_slots` ensures the wire extends far enough to cover every
/// placed gate even when the rightmost gate sits past the canvas's
/// natural `width - LINE_RIGHT_OFFSET` boundary. Callers compute it
/// from `placed_gates` (e.g. `max_slot_index + 2` so the trailing
/// empty drop-target slot stays visible). Passing `0` keeps the old
/// canvas-width-only behaviour.
pub(crate) fn layout_metrics(width: f32, qubit_count: usize, min_slots: usize) -> LayoutMetrics {
    let line_left = LINE_LEFT_OFFSET;
    let canvas_line_right = width - LINE_RIGHT_OFFSET;
    let line_ys = (0..qubit_count)
        .map(|index| LINE_Y + LINE_GAP * index as f32)
        .collect::<Vec<f32>>();
    let slot_left = line_left + GATE_SIZE;
    let canvas_slot_right = canvas_line_right - GATE_SIZE;
    let canvas_slots = if SLOT_SPACING > 0.0 {
        (((canvas_slot_right - slot_left) / SLOT_SPACING).floor() as i32 + 1).max(0) as usize
    } else {
        0
    };
    // Take whichever is larger: the slots that naturally fit in the
    // canvas, or the slots demanded by the placed-gate set. Wires +
    // slot_centers grow with the larger number.
    let slot_count = canvas_slots.max(min_slots);
    let slot_centers = if slot_count > 0 {
        (0..slot_count)
            .map(|index| slot_left + SLOT_SPACING * index as f32)
            .collect::<Vec<f32>>()
    } else {
        Vec::new()
    };
    let slot_right = slot_centers.last().copied().unwrap_or(slot_left);
    // Wires terminate one GATE_SIZE past the rightmost slot center so
    // the last gate's body sits comfortably inside the line, mirroring
    // the original canvas-width-based formula.
    let line_right = slot_right + GATE_SIZE;
    LayoutMetrics {
        line_left,
        line_right,
        line_ys,
        slot_left,
        slot_right,
        slot_centers,
    }
}

pub(crate) fn nearest_slot_index(x: f32, slot_centers: &[f32]) -> Option<(usize, f32)> {
    let mut nearest_index = None;
    let mut nearest_distance = f32::MAX;
    for (index, &slot) in slot_centers.iter().enumerate() {
        let distance = (x - slot).abs();
        if distance < nearest_distance {
            nearest_distance = distance;
            nearest_index = Some(index);
        }
    }
    nearest_index.map(|index| (index, nearest_distance))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SlotSnap {
    pub(crate) index: usize,
    pub(crate) center: f32,
    pub(crate) distance: f32,
}

pub(crate) fn nearest_available_slot(
    x: f32,
    wire_index: usize,
    ignore_id: Option<u32>,
    gates: &[PlacedGate],
    slot_centers: &[f32],
) -> Option<SlotSnap> {
    let mut occupied_columns = Vec::new();
    for gate in gates {
        if gate.wire != wire_index {
            continue;
        }
        if ignore_id == Some(gate.id) {
            continue;
        }
        occupied_columns.push(gate.column);
    }

    let mut nearest = None;
    let mut nearest_distance = f32::MAX;
    for (index, &center) in slot_centers.iter().enumerate() {
        if occupied_columns.contains(&index) {
            continue;
        }
        let distance = (x - center).abs();
        if nearest.is_none() || distance < nearest_distance {
            nearest = Some(SlotSnap {
                index,
                center,
                distance,
            });
            nearest_distance = distance;
        }
    }
    nearest
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PaletteLayout {
    pub(crate) total_width: f32,
    pub(crate) total_height: f32,
}

fn palette_row_width(count: usize) -> f32 {
    if count == 0 {
        0.0
    } else {
        count as f32 * PALETTE_SIZE + (count - 1) as f32 * PALETTE_GAP
    }
}

pub(crate) fn palette_layout() -> PaletteLayout {
    let row1_count = PALETTE_ROW1_COUNT;
    let row2_count = PALETTE_GATES.len().saturating_sub(row1_count);
    let row1_width = palette_row_width(row1_count);
    let row2_width = palette_row_width(row2_count);
    PaletteLayout {
        total_width: row1_width.max(row2_width),
        total_height: 2.0 * PALETTE_SIZE + PALETTE_ROW_GAP,
    }
}

fn palette_row_for_index(index: usize) -> Option<(usize, usize)> {
    if index < PALETTE_ROW1_COUNT {
        Some((0, index))
    } else if index < PALETTE_GATES.len() {
        Some((1, index - PALETTE_ROW1_COUNT))
    } else {
        None
    }
}

/// Returns gate top-left position relative to the palette panel top-left.
/// Both rows are left-aligned, matching qni's `flex flex-row` layout.
pub(crate) fn palette_gate_local_pos(index: usize, _layout: &PaletteLayout) -> Option<egui::Pos2> {
    let (row, col) = palette_row_for_index(index)?;
    let x = col as f32 * (PALETTE_SIZE + PALETTE_GAP);
    let y = row as f32 * (PALETTE_SIZE + PALETTE_ROW_GAP);
    Some(egui::pos2(x, y))
}

/// Maps a cursor position (relative to the palette panel top-left) to a flat
/// gate index. Returns `None` outside any gate cell.
pub(crate) fn palette_hit_test(local_pos: egui::Pos2, _layout: &PaletteLayout) -> Option<usize> {
    if local_pos.y < 0.0 || local_pos.x < 0.0 {
        return None;
    }
    let row = if local_pos.y <= PALETTE_SIZE {
        0
    } else if local_pos.y >= PALETTE_SIZE + PALETTE_ROW_GAP
        && local_pos.y <= 2.0 * PALETTE_SIZE + PALETTE_ROW_GAP
    {
        1
    } else {
        return None;
    };
    let row_count = if row == 0 {
        PALETTE_ROW1_COUNT
    } else {
        PALETTE_GATES.len() - PALETTE_ROW1_COUNT
    };
    let col_index = (local_pos.x / (PALETTE_SIZE + PALETTE_GAP)).floor() as i32;
    if col_index < 0 || (col_index as usize) >= row_count {
        return None;
    }
    let col_offset = local_pos.x - col_index as f32 * (PALETTE_SIZE + PALETTE_GAP);
    if col_offset > PALETTE_SIZE {
        return None;
    }
    let flat_index = if row == 0 {
        col_index as usize
    } else {
        PALETTE_ROW1_COUNT + col_index as usize
    };
    Some(flat_index)
}

pub(crate) fn nearest_line(y: f32, line_ys: &[f32]) -> (f32, f32, usize) {
    let mut nearest = line_ys[0];
    let mut nearest_distance = (y - line_ys[0]).abs();
    let mut nearest_index = 0;
    for (index, &line_y) in line_ys.iter().enumerate() {
        let distance = (y - line_y).abs();
        if distance < nearest_distance {
            nearest = line_y;
            nearest_distance = distance;
            nearest_index = index;
        }
    }
    (nearest, nearest_distance, nearest_index)
}
