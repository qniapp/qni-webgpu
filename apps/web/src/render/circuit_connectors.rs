//! Circuit connector and phase-label drawing facade.
//!
//! Connector families are split by gate relationship: controlled columns,
//! swap pairs, and same-angle phase chains / labels.

mod control;
pub(in crate::render) mod phase;
mod swap;

use eframe::egui;

use crate::app::{GateId, QniApp};
use crate::colors::Colors;
use crate::layout::{CircuitProjection, LayoutMetrics};

// 4px. Use the same even-width body for all vertical
// gate connectors (Control / Swap / same-angle Phase) so the connector shares
// the exact same center coordinate as the 40px gate grid.
pub(super) const CONNECTOR_STROKE_WIDTH: f32 = crate::tailwind::SPACING_1;

/// Screen coordinate of `point` along the wire axis (across the wires).
pub(super) fn along_wires(projection: CircuitProjection, point: egui::Pos2) -> f32 {
    if projection.is_vertical() {
        point.x
    } else {
        point.y
    }
}

/// Screen coordinate of `point` along the time axis.
pub(super) fn along_time(projection: CircuitProjection, point: egui::Pos2) -> f32 {
    if projection.is_vertical() {
        point.y
    } else {
        point.x
    }
}

/// Screen coordinate along the time axis of circuit-space time `x`.
pub(super) fn screen_time(projection: CircuitProjection, x: f32) -> f32 {
    along_time(projection, projection.to_screen(egui::pos2(x, 0.0)))
}

/// Connector across the wires at screen time `time`, from screen
/// coordinate `start` to `end` along the wire axis.
pub(super) fn draw_wire_connector(
    painter: &egui::Painter,
    projection: CircuitProjection,
    time: f32,
    start: f32,
    end: f32,
    color: egui::Color32,
) {
    let (low, high) = if start <= end {
        (start, end)
    } else {
        (end, start)
    };
    let half_width = CONNECTOR_STROKE_WIDTH * 0.5;
    let rect = if projection.is_vertical() {
        egui::Rect::from_min_max(
            egui::pos2(low, time - half_width),
            egui::pos2(high, time + half_width),
        )
    } else {
        egui::Rect::from_min_max(
            egui::pos2(time - half_width, low),
            egui::pos2(time + half_width, high),
        )
    };
    painter.rect_filled(rect, egui::CornerRadius::ZERO, color);
}

impl QniApp {
    pub(super) fn draw_circuit_connectors(
        &self,
        painter: &egui::Painter,
        metrics: &LayoutMetrics,
        colors: &Colors,
        projection: CircuitProjection,
        dragging_gate_id: Option<GateId>,
    ) {
        // Connector lines are computed every frame, including mid-drag, so a
        // gate being moved into or out of a multi-qubit relationship snaps
        // visually before drop. The work is cheap: one pass per connector
        // family over the ≤16-qubit circuit.
        control::draw_control_connectors(
            self,
            painter,
            metrics,
            colors,
            projection,
            dragging_gate_id,
        );
        swap::draw_swap_connectors(self, painter, metrics, colors, projection, dragging_gate_id);
        phase::draw_phase_connectors_and_labels(
            self,
            painter,
            metrics,
            colors,
            projection,
            self.dragging
                .filter(|drag| Some(drag.id) == dragging_gate_id),
        );
    }
}
