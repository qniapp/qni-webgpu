use eframe::egui;

use crate::app::{PlacedGate, SpanResizeEdge};
use crate::constants::{
    GATE_SIZE, LINE_GAP, LINE_LEFT_OFFSET, LINE_RIGHT_OFFSET, LINE_Y, QFT_RESIZE_HANDLE_HEIGHT,
    QFT_RESIZE_HANDLE_WIDTH, SLOT_SPACING,
};
use crate::gates::GateKind;

/// Visible rect of a placed gate, accounting for the multi-qubit `span`
/// of resizable-span gates. Single-qubit gates get `GATE_SIZE` ×
/// `GATE_SIZE`; QFT / Chance extend downward to cover all wires in the span.
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

/// Resize-handle bounding box for any resizable-span gate edge. Chance uses
/// two spec-sized pills (24×6 px) centred 9 px outside the top / bottom edge;
/// QFT keeps the existing bottom chevron strip.
pub(crate) fn span_resize_handle_rect(
    kind: GateKind,
    gate_rect: egui::Rect,
    edge: SpanResizeEdge,
) -> egui::Rect {
    if kind != GateKind::ChanceDisplay {
        return qft_resize_handle_rect(gate_rect);
    }
    let cx = gate_rect.center().x;
    let cy = match edge {
        SpanResizeEdge::Top => gate_rect.top() - 9.0,
        SpanResizeEdge::Bottom => gate_rect.bottom() + 9.0,
    };
    egui::Rect::from_center_size(egui::pos2(cx, cy), egui::vec2(24.0, 6.0))
}

fn span_resize_handle_hit_rect(
    kind: GateKind,
    gate_rect: egui::Rect,
    edge: SpanResizeEdge,
) -> egui::Rect {
    let rect = span_resize_handle_rect(kind, gate_rect, edge);
    if kind != GateKind::ChanceDisplay {
        return rect;
    }
    // The visible Chance pill scales up to 1.25× while hovered/active;
    // keep the pointer target matched to the largest visual state and extend
    // it through the visual gap so the handle does not disappear while the
    // pointer travels from the gate body to the pill.
    let expanded = rect.expand2(egui::vec2(3.0, 1.0));
    match edge {
        SpanResizeEdge::Top => {
            egui::Rect::from_min_max(expanded.min, egui::pos2(expanded.max.x, gate_rect.top()))
        }
        SpanResizeEdge::Bottom => {
            egui::Rect::from_min_max(egui::pos2(expanded.min.x, gate_rect.bottom()), expanded.max)
        }
    }
}

pub(crate) fn span_resize_handle_edge_at(
    kind: GateKind,
    gate_rect: egui::Rect,
    cursor: egui::Pos2,
) -> Option<SpanResizeEdge> {
    if kind != GateKind::ChanceDisplay {
        return span_resize_handle_hit_rect(kind, gate_rect, SpanResizeEdge::Bottom)
            .contains(cursor)
            .then_some(SpanResizeEdge::Bottom);
    }
    [SpanResizeEdge::Top, SpanResizeEdge::Bottom]
        .into_iter()
        .find(|&edge| span_resize_handle_hit_rect(kind, gate_rect, edge).contains(cursor))
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
