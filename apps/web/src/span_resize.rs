//! Shared resizable-span handle component for Probability / QFT / QFT† / Amplitude / Density gates.
//!
//! This module owns the small cyan top/bottom handles as one component:
//! body selection, hit testing, drag construction, animation easing, and paint.
//! Gate-specific code only decides whether a gate is resizable and how far the
//! span may grow.

use eframe::egui;

use crate::app::{PlacedGate, SpanResizeDrag, SpanResizeEdge, SpanResizeHandle, WireIndex};
use crate::colors::Colors;
use crate::gates::GateKind;
use crate::icons::draw_span_resize_handle;
use crate::layout::{amplitude_grid_rect, gate_rect_at_grid, gate_visible_rect, gate_width_cols};

const SPAN_RESIZE_HANDLE_MIN_WIDTH: f32 = 24.0;
const SPAN_RESIZE_HANDLE_WIDTH_RATIO: f32 = 0.6;
const SPAN_RESIZE_HANDLE_HEIGHT: f32 = 6.0;
const SPAN_RESIZE_HANDLE_MAX_SCALE_PAD_RATIO: f32 = 0.125;
const SPAN_RESIZE_HANDLE_EDGES: [SpanResizeEdge; 2] = [SpanResizeEdge::Top, SpanResizeEdge::Bottom];

pub(crate) fn span_resize_ease_out_back(t: f32) -> f32 {
    const C1: f32 = 1.56;
    const C3: f32 = C1 + 1.0;
    let u = t - 1.0;
    1.0 + C3 * u * u * u + C1 * u * u
}

/// The visual body that owns the resize handles. Amplitude uses the centred
/// matrix draw area; Density uses its square canvas (the whole visible rect),
/// and fixed-width gates clamp the 60% policy to the 24 px minimum.
pub(crate) fn span_resize_body_rect(
    kind: GateKind,
    span: usize,
    gate_rect: egui::Rect,
) -> egui::Rect {
    if kind == GateKind::AmplitudeDisplay {
        amplitude_grid_rect(gate_rect, span)
    } else {
        gate_rect
    }
}

/// Resize-handle bounding box for any resizable-span gate edge. The 6 px-tall
/// pill keeps the Probability-display 24 px minimum, then scales to 60% of the
/// supplied body width for wide Amplitude / Density displays.
fn span_resize_handle_rect(body_rect: egui::Rect, edge: SpanResizeEdge) -> egui::Rect {
    let cx = body_rect.center().x;
    let cy = match edge {
        SpanResizeEdge::Top => body_rect.top() - 9.0,
        SpanResizeEdge::Bottom => body_rect.bottom() + 9.0,
    };
    let width = (body_rect.width() * SPAN_RESIZE_HANDLE_WIDTH_RATIO)
        .round()
        .max(SPAN_RESIZE_HANDLE_MIN_WIDTH);
    egui::Rect::from_center_size(
        egui::pos2(cx, cy),
        egui::vec2(width, SPAN_RESIZE_HANDLE_HEIGHT),
    )
}

fn span_resize_handle_hit_rect(body_rect: egui::Rect, edge: SpanResizeEdge) -> egui::Rect {
    let rect = span_resize_handle_rect(body_rect, edge);
    // The visible pill scales up to 1.25× while hovered/active; keep the
    // pointer target matched to the largest visual state. Horizontal padding
    // is `(1.25 - 1.0) / 2` of the unscaled width, with the old 24 px handle's
    // 3 px padding retained as a floor.
    let expanded = rect.expand2(egui::vec2(
        (rect.width() * SPAN_RESIZE_HANDLE_MAX_SCALE_PAD_RATIO).max(3.0),
        1.0,
    ));
    match edge {
        SpanResizeEdge::Top => {
            egui::Rect::from_min_max(expanded.min, egui::pos2(expanded.max.x, body_rect.top()))
        }
        SpanResizeEdge::Bottom => {
            egui::Rect::from_min_max(egui::pos2(expanded.min.x, body_rect.bottom()), expanded.max)
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SpanResizeHandles {
    gate_id: u32,
    body_rect: egui::Rect,
    available_edges: [bool; 2],
}

impl SpanResizeHandles {
    pub(crate) fn for_gate_with_availability(
        gate: &PlacedGate,
        gates: &[PlacedGate],
        qubit_capacity: usize,
    ) -> Option<Self> {
        Self::for_gate_at_with_availability(
            gate,
            gate_visible_rect(gate, gate.pos),
            gates,
            qubit_capacity,
        )
    }

    pub(crate) fn for_gate_at_with_availability(
        gate: &PlacedGate,
        gate_rect: egui::Rect,
        gates: &[PlacedGate],
        qubit_capacity: usize,
    ) -> Option<Self> {
        if !gate.kind.is_resizable_span() {
            return None;
        }
        Some(Self {
            gate_id: gate.id,
            body_rect: span_resize_body_rect(gate.kind, gate.span, gate_rect),
            available_edges: available_edges(gate, gates, qubit_capacity),
        })
    }

    pub(crate) fn body_rect(self) -> egui::Rect {
        self.body_rect
    }

    pub(crate) fn edge_at(self, cursor: egui::Pos2) -> Option<SpanResizeEdge> {
        SPAN_RESIZE_HANDLE_EDGES.into_iter().find(|&edge| {
            self.edge_available(edge)
                && span_resize_handle_hit_rect(self.body_rect, edge).contains(cursor)
        })
    }

    fn edge_available(self, edge: SpanResizeEdge) -> bool {
        self.available_edges[edge_index(edge)]
    }

    pub(crate) fn handle(self, edge: SpanResizeEdge) -> SpanResizeHandle {
        SpanResizeHandle {
            gate_id: self.gate_id,
            edge,
        }
    }

    pub(crate) fn drag_at(self, gate: &PlacedGate, cursor: egui::Pos2) -> Option<SpanResizeDrag> {
        let edge = self.edge_at(cursor)?;
        Some(SpanResizeDrag {
            gate_id: gate.id,
            edge,
            start_pointer_y: cursor.y,
            start_wire: gate.wire.as_usize(),
            start_span: gate.span.max(1),
        })
    }

    pub(crate) fn paint(
        self,
        painter: &egui::Painter,
        colors: &Colors,
        hovered_handle: Option<SpanResizeHandle>,
        active_drag: Option<SpanResizeDrag>,
        visible_t: f32,
    ) {
        for edge in SPAN_RESIZE_HANDLE_EDGES {
            let handle = self.handle(edge);
            let hovered = hovered_handle == Some(handle);
            let active =
                active_drag.is_some_and(|drag| drag.gate_id == self.gate_id && drag.edge == edge);
            if !self.edge_available(edge) && !active {
                continue;
            }
            let bg = if hovered || active {
                colors.span_resize_handle_bg_hover
            } else {
                colors.span_resize_handle_bg
            };
            let scale = if active {
                1.25
            } else if hovered {
                1.15
            } else {
                0.7 + 0.3 * visible_t
            };
            let alpha = if hovered || active { 1.0 } else { visible_t };
            draw_span_resize_handle(
                painter,
                span_resize_handle_rect(self.body_rect, edge),
                bg,
                scale,
                alpha,
            );
        }
    }
}

fn edge_index(edge: SpanResizeEdge) -> usize {
    match edge {
        SpanResizeEdge::Top => 0,
        SpanResizeEdge::Bottom => 1,
    }
}

fn available_edges(gate: &PlacedGate, gates: &[PlacedGate], qubit_capacity: usize) -> [bool; 2] {
    [
        edge_can_change_span(gate, gates, qubit_capacity, SpanResizeEdge::Top),
        edge_can_change_span(gate, gates, qubit_capacity, SpanResizeEdge::Bottom),
    ]
}

fn edge_can_change_span(
    gate: &PlacedGate,
    gates: &[PlacedGate],
    qubit_capacity: usize,
    edge: SpanResizeEdge,
) -> bool {
    match edge {
        SpanResizeEdge::Top => {
            can_shrink_from_top(gate, gates, qubit_capacity)
                || can_grow_from_top(gate, gates, qubit_capacity)
        }
        SpanResizeEdge::Bottom => {
            can_shrink_from_bottom(gate, gates, qubit_capacity)
                || can_grow_from_bottom(gate, gates, qubit_capacity)
        }
    }
}

fn can_shrink_from_top(gate: &PlacedGate, gates: &[PlacedGate], qubit_capacity: usize) -> bool {
    gate.span > 1
        && candidate_span_is_clear(
            gate,
            gates,
            gate.wire.as_usize() + 1,
            gate.span - 1,
            qubit_capacity,
        )
}

fn can_shrink_from_bottom(gate: &PlacedGate, gates: &[PlacedGate], qubit_capacity: usize) -> bool {
    gate.span > 1
        && candidate_span_is_clear(
            gate,
            gates,
            gate.wire.as_usize(),
            gate.span - 1,
            qubit_capacity,
        )
}

fn can_grow_from_top(gate: &PlacedGate, gates: &[PlacedGate], qubit_capacity: usize) -> bool {
    let bottom_wire = gate.wire.as_usize() + gate.span.saturating_sub(1);
    let next_span = gate.span + 1;
    if gate.wire.as_usize() == 0 || gate.kind.max_resizable_span(bottom_wire + 1) < next_span {
        return false;
    }
    candidate_span_is_clear(
        gate,
        gates,
        gate.wire.as_usize() - 1,
        next_span,
        qubit_capacity,
    )
}

fn can_grow_from_bottom(gate: &PlacedGate, gates: &[PlacedGate], qubit_capacity: usize) -> bool {
    let remaining_wires = qubit_capacity.saturating_sub(gate.wire.as_usize()).max(1);
    let next_span = gate.span + 1;
    if gate.kind.max_resizable_span(remaining_wires) < next_span {
        return false;
    }
    candidate_span_is_clear(gate, gates, gate.wire.as_usize(), next_span, qubit_capacity)
}

pub(crate) fn resolve_span_resize_candidate(
    gate: &PlacedGate,
    gates: &[PlacedGate],
    qubit_capacity: usize,
    edge: SpanResizeEdge,
    desired_wire: usize,
    desired_span: usize,
) -> (usize, usize) {
    match edge {
        SpanResizeEdge::Bottom => {
            resolve_bottom_resize_candidate(gate, gates, qubit_capacity, desired_span.max(1))
        }
        SpanResizeEdge::Top => {
            resolve_top_resize_candidate(gate, gates, qubit_capacity, desired_wire)
        }
    }
}

fn resolve_bottom_resize_candidate(
    gate: &PlacedGate,
    gates: &[PlacedGate],
    qubit_capacity: usize,
    desired_span: usize,
) -> (usize, usize) {
    let mut span = gate.span;
    if desired_span <= gate.span {
        for next_span in (desired_span..gate.span).rev() {
            if !candidate_span_is_clear(
                gate,
                gates,
                gate.wire.as_usize(),
                next_span,
                qubit_capacity,
            ) {
                break;
            }
            span = next_span;
        }
        return (gate.wire.as_usize(), span);
    }

    for next_span in (gate.span + 1)..=desired_span {
        if !candidate_span_is_clear(gate, gates, gate.wire.as_usize(), next_span, qubit_capacity) {
            break;
        }
        span = next_span;
    }
    (gate.wire.as_usize(), span)
}

fn resolve_top_resize_candidate(
    gate: &PlacedGate,
    gates: &[PlacedGate],
    qubit_capacity: usize,
    desired_wire: usize,
) -> (usize, usize) {
    let bottom_wire = gate.wire.as_usize() + gate.span.saturating_sub(1);
    let mut wire = gate.wire.as_usize();
    let mut span = gate.span;
    if desired_wire >= gate.wire.as_usize() {
        let target_wire = desired_wire.min(bottom_wire);
        for next_wire in (gate.wire.as_usize() + 1)..=target_wire {
            let next_span = bottom_wire - next_wire + 1;
            if !candidate_span_is_clear(gate, gates, next_wire, next_span, qubit_capacity) {
                break;
            }
            wire = next_wire;
            span = next_span;
        }
        return (wire, span);
    }

    for next_wire in (desired_wire..gate.wire.as_usize()).rev() {
        let next_span = bottom_wire - next_wire + 1;
        if !candidate_span_is_clear(gate, gates, next_wire, next_span, qubit_capacity) {
            break;
        }
        wire = next_wire;
        span = next_span;
    }
    (wire, span)
}

fn candidate_span_is_clear(
    gate: &PlacedGate,
    gates: &[PlacedGate],
    candidate_wire: usize,
    candidate_span: usize,
    qubit_capacity: usize,
) -> bool {
    if candidate_wire + candidate_span > qubit_capacity {
        return false;
    }
    let candidate_rect = gate_rect_at_grid(
        gate.kind,
        gate.column,
        WireIndex::new(candidate_wire),
        candidate_span,
    );
    let old_width = gate_width_cols(gate.kind, gate.span);
    let new_width = gate_width_cols(gate.kind, candidate_span);
    let old_right = gate
        .column
        .checked_add(old_width)
        .map(|column| column.as_usize());
    let Some(other_rects) = gates
        .iter()
        .filter(|other| other.id != gate.id)
        .map(|other| {
            let shifted_column =
                shifted_column_after_width_change(other, old_right, old_width, new_width)?;
            Some(gate_rect_at_grid(
                other.kind,
                shifted_column,
                other.wire,
                other.span,
            ))
        })
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    if other_rects
        .iter()
        .any(|other_rect| candidate_rect.intersects(*other_rect))
    {
        return false;
    }
    for left in 0..other_rects.len() {
        for right in (left + 1)..other_rects.len() {
            if other_rects[left].intersects(other_rects[right]) {
                return false;
            }
        }
    }
    true
}

fn shifted_column_after_width_change(
    gate: &PlacedGate,
    old_right: Option<usize>,
    old_width: usize,
    new_width: usize,
) -> Option<crate::app::CircuitColumnIndex> {
    let old_right = old_right?;
    if gate.column.as_usize() < old_right {
        return Some(gate.column);
    }
    if new_width > old_width {
        gate.column.checked_add(new_width - old_width)
    } else {
        Some(gate.column.saturating_sub(old_width - new_width))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::CircuitColumnIndex;
    use crate::constants::{GATE_SIZE, SLOT_SPACING};

    fn placed_gate(id: u32, kind: GateKind, wire: usize, span: usize) -> PlacedGate {
        placed_gate_at(id, kind, 0, wire, span)
    }

    fn placed_gate_at(
        id: u32,
        kind: GateKind,
        column: usize,
        wire: usize,
        span: usize,
    ) -> PlacedGate {
        PlacedGate::new(
            id,
            kind,
            CircuitColumnIndex::new(column),
            WireIndex::new(wire),
            span,
            None,
        )
    }

    #[test]
    fn amplitude_body_rect_uses_matrix_draw_area() {
        let rect = egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(GATE_SIZE + SLOT_SPACING, GATE_SIZE),
        );

        let body = span_resize_body_rect(GateKind::AmplitudeDisplay, 1, rect);

        assert_eq!((body.width(), body.height()), (80.0, 40.0));
    }

    #[test]
    fn probability_body_rect_uses_whole_gate_rect() {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(GATE_SIZE, 264.0));

        assert_eq!(
            span_resize_body_rect(GateKind::ProbabilityDisplay, 5, rect),
            rect
        );
    }

    #[test]
    fn handle_width_keeps_probability_display_minimum() {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(GATE_SIZE, GATE_SIZE));

        assert_eq!(
            span_resize_handle_rect(rect, SpanResizeEdge::Top).width(),
            24.0
        );
    }

    #[test]
    fn handle_width_matches_mock_span_one_body() {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(80.0, GATE_SIZE));

        assert_eq!(
            span_resize_handle_rect(rect, SpanResizeEdge::Top).width(),
            48.0
        );
    }

    #[test]
    fn handle_width_matches_mock_span_four_body() {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(208.0, 208.0));

        assert_eq!(
            span_resize_handle_rect(rect, SpanResizeEdge::Top).width(),
            125.0
        );
    }

    #[test]
    fn handle_width_matches_mock_span_six_bottom_body() {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(320.0, 320.0));

        assert_eq!(
            span_resize_handle_rect(rect, SpanResizeEdge::Bottom).width(),
            192.0
        );
    }

    #[test]
    fn span_one_top_handle_is_unavailable_on_first_wire() {
        let gate = placed_gate(1, GateKind::QftGate, 0, 1);
        let handles =
            SpanResizeHandles::for_gate_with_availability(&gate, std::slice::from_ref(&gate), 2)
                .expect("QFT is resizable");

        assert_eq!(handles.available_edges, [false, true]);
    }

    #[test]
    fn span_one_bottom_handle_is_unavailable_when_adjacent_wire_is_occupied() {
        let gate = placed_gate(1, GateKind::QftGate, 0, 1);
        let blocker = placed_gate(2, GateKind::H, 1, 1);
        let handles =
            SpanResizeHandles::for_gate_with_availability(&gate, &[gate.clone(), blocker], 2)
                .expect("QFT is resizable");

        assert_eq!(handles.available_edges, [false, false]);
    }

    #[test]
    fn span_two_handles_remain_available_for_shrinking_at_capacity_edges() {
        let gate = placed_gate(1, GateKind::QftGate, 0, 2);
        let handles =
            SpanResizeHandles::for_gate_with_availability(&gate, std::slice::from_ref(&gate), 2)
                .expect("QFT is resizable");

        assert_eq!(handles.available_edges, [true, true]);
    }

    #[test]
    fn density_span_one_bottom_handle_allows_trailing_column_push() {
        let gate = placed_gate_at(1, GateKind::DensityMatrixDisplay, 0, 0, 1);
        let trailing = placed_gate_at(2, GateKind::H, 1, 1, 1);
        let handles =
            SpanResizeHandles::for_gate_with_availability(&gate, &[gate.clone(), trailing], 2)
                .expect("Density display is resizable");

        assert_eq!(handles.available_edges, [false, true]);
    }

    #[test]
    fn density_bottom_resize_stops_before_unshifted_lower_blocker() {
        let gate = placed_gate_at(1, GateKind::DensityMatrixDisplay, 0, 0, 2);
        let blocker = placed_gate_at(2, GateKind::H, 1, 2, 1);
        let resolved = resolve_span_resize_candidate(
            &gate,
            &[gate.clone(), blocker],
            3,
            SpanResizeEdge::Bottom,
            0,
            3,
        );

        assert_eq!(resolved, (0, 2));
    }

    #[test]
    fn density_handles_are_unavailable_when_shrink_and_grow_are_blocked() {
        let gate = placed_gate_at(1, GateKind::DensityMatrixDisplay, 0, 0, 2);
        let stationary = placed_gate_at(2, GateKind::H, 1, 2, 1);
        let trailing = placed_gate_at(3, GateKind::H, 2, 2, 1);
        let handles = SpanResizeHandles::for_gate_with_availability(
            &gate,
            &[gate.clone(), stationary, trailing],
            3,
        )
        .expect("Density display is resizable");

        assert_eq!(handles.available_edges, [false, false]);
    }

    #[test]
    fn density_shrink_stops_before_shifted_trailing_gate_collision() {
        let gate = placed_gate_at(1, GateKind::DensityMatrixDisplay, 0, 0, 2);
        let stationary = placed_gate_at(2, GateKind::H, 1, 2, 1);
        let trailing = placed_gate_at(3, GateKind::H, 2, 2, 1);
        let resolved = resolve_span_resize_candidate(
            &gate,
            &[gate.clone(), stationary, trailing],
            3,
            SpanResizeEdge::Bottom,
            0,
            1,
        );

        assert_eq!(resolved, (0, 2));
    }

    #[test]
    fn density_multi_step_shrink_stops_before_intermediate_shift_collision() {
        let gate = placed_gate_at(1, GateKind::DensityMatrixDisplay, 0, 0, 3);
        let stationary = placed_gate_at(2, GateKind::H, 2, 3, 1);
        let trailing = placed_gate_at(3, GateKind::H, 3, 3, 1);
        let resolved = resolve_span_resize_candidate(
            &gate,
            &[gate.clone(), stationary, trailing],
            4,
            SpanResizeEdge::Bottom,
            0,
            1,
        );

        assert_eq!(resolved, (0, 3));
    }

    #[test]
    fn bottom_resize_stops_before_occupied_lower_wire() {
        let gate = placed_gate(1, GateKind::QftGate, 0, 3);
        let blocker = placed_gate(2, GateKind::H, 3, 1);
        let resolved = resolve_span_resize_candidate(
            &gate,
            &[gate.clone(), blocker],
            4,
            SpanResizeEdge::Bottom,
            0,
            4,
        );

        assert_eq!(resolved, (0, 3));
    }

    #[test]
    fn bottom_resize_can_shrink_when_lower_wire_is_occupied() {
        let gate = placed_gate(1, GateKind::QftGate, 0, 3);
        let blocker = placed_gate(2, GateKind::H, 3, 1);
        let resolved = resolve_span_resize_candidate(
            &gate,
            &[gate.clone(), blocker],
            4,
            SpanResizeEdge::Bottom,
            0,
            2,
        );

        assert_eq!(resolved, (0, 2));
    }

    #[test]
    fn top_resize_stops_before_occupied_upper_wire() {
        let gate = placed_gate(1, GateKind::QftGate, 1, 3);
        let blocker = placed_gate(2, GateKind::H, 0, 1);
        let resolved = resolve_span_resize_candidate(
            &gate,
            &[gate.clone(), blocker],
            4,
            SpanResizeEdge::Top,
            0,
            4,
        );

        assert_eq!(resolved, (1, 3));
    }
}
