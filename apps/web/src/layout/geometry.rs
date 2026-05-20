use eframe::egui;

use crate::app::PlacedGate;
use crate::constants::{
    GATE_SIZE, LINE_GAP, LINE_LEFT_OFFSET, LINE_RIGHT_OFFSET, LINE_Y, SLOT_SPACING,
};
use crate::gates::GateKind;

pub(crate) fn amplitude_display_width_cols(span: usize) -> usize {
    let span = span.clamp(1, 16);
    if span == 1 {
        2
    } else if span.is_multiple_of(2) {
        span
    } else {
        span.div_ceil(2)
    }
}

pub(crate) fn amplitude_grid_dims(span: usize) -> (usize, usize) {
    let span = span.clamp(1, 16);
    let outcomes = 1usize << span;
    let width = if outcomes == 2 {
        2
    } else {
        1usize << (span / 2)
    };
    (width, outcomes / width)
}

pub(crate) fn gate_width_cols(kind: GateKind, span: usize) -> usize {
    match kind {
        GateKind::AmplitudeDisplay => amplitude_display_width_cols(span),
        _ => 1,
    }
}

pub(crate) fn gate_size(kind: GateKind, span: usize) -> egui::Vec2 {
    let width_cols = gate_width_cols(kind, span).max(1);
    let width = (width_cols - 1) as f32 * SLOT_SPACING + GATE_SIZE;
    let height = if kind.is_resizable_span() {
        let span = span.max(1);
        (span - 1) as f32 * LINE_GAP + GATE_SIZE
    } else {
        GATE_SIZE
    };
    egui::vec2(width, height)
}

pub(crate) fn gate_rect_at_grid(
    kind: GateKind,
    column: usize,
    wire: usize,
    span: usize,
) -> egui::Rect {
    egui::Rect::from_min_size(PlacedGate::grid_pos(column, wire), gate_size(kind, span))
}

/// Visible rect of a placed gate, accounting for the multi-qubit `span`
/// of resizable-span gates and the variable-width Amplitude display body.
/// `origin` is the top-left of the gate body (= rect.min + gate.pos
/// in the circuit's local coordinate space).
pub(crate) fn gate_visible_rect(gate: &PlacedGate, origin: egui::Pos2) -> egui::Rect {
    egui::Rect::from_min_size(origin, gate_size(gate.kind, gate.span))
}

pub(crate) fn amplitude_grid_rect(gate_rect: egui::Rect, span: usize) -> egui::Rect {
    let (cols, rows) = amplitude_grid_dims(span);
    let cell = (gate_rect.width() / cols as f32).min(gate_rect.height() / rows as f32);
    let size = egui::vec2(cell * cols as f32, cell * rows as f32);
    egui::Rect::from_center_size(gate_rect.center(), size)
}

pub(crate) fn amplitude_cell_index_at(
    gate_rect: egui::Rect,
    span: usize,
    cursor: egui::Pos2,
) -> Option<u32> {
    let grid_rect = amplitude_grid_rect(gate_rect, span);
    if !grid_rect.contains(cursor) {
        return None;
    }
    let (cols, rows) = amplitude_grid_dims(span);
    let cell = grid_rect.width() / cols as f32;
    let col = ((cursor.x - grid_rect.left()) / cell)
        .floor()
        .clamp(0.0, (cols - 1) as f32) as usize;
    let row = ((cursor.y - grid_rect.top()) / cell)
        .floor()
        .clamp(0.0, (rows - 1) as f32) as usize;
    Some((row * cols + col) as u32)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amplitude_width_cols_follow_spec() {
        let widths = (1..=5)
            .map(amplitude_display_width_cols)
            .collect::<Vec<_>>();

        assert_eq!(widths, vec![2, 2, 2, 4, 3]);
    }

    #[test]
    fn amplitude_span_five_size_is_three_columns_by_five_wires() {
        let size = gate_size(GateKind::AmplitudeDisplay, 5);

        assert_eq!(
            (size.x, size.y),
            (GATE_SIZE + SLOT_SPACING * 2.0, GATE_SIZE + LINE_GAP * 4.0)
        );
    }

    #[test]
    fn amplitude_span_three_grid_is_two_by_four() {
        assert_eq!(amplitude_grid_dims(3), (2, 4));
    }
}
