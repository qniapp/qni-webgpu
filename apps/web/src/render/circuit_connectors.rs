//! Circuit connector and phase-label drawing facade.
//!
//! Connector families are split by gate relationship: controlled columns,
//! swap pairs, and same-angle phase chains / labels.

mod control;
pub(in crate::render) mod phase;
mod swap;

use eframe::egui;

use crate::app::QniApp;
use crate::colors::Colors;
use crate::layout::LayoutMetrics;

// Tailwind spacing-1 = 4px. Use the same even-width body for all vertical
// gate connectors (Control / Swap / same-angle Phase) so the connector shares
// the exact same center coordinate as the 40px gate grid.
pub(super) const CONNECTOR_STROKE_WIDTH: f32 = 4.0;

pub(super) fn draw_vertical_connector(
    painter: &egui::Painter,
    x: f32,
    start_y: f32,
    end_y: f32,
    color: egui::Color32,
) {
    let (top, bottom) = if start_y <= end_y {
        (start_y, end_y)
    } else {
        (end_y, start_y)
    };
    let half_width = CONNECTOR_STROKE_WIDTH * 0.5;
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(x - half_width, top),
            egui::pos2(x + half_width, bottom),
        ),
        egui::CornerRadius::ZERO,
        color,
    );
}

impl QniApp {
    pub(super) fn draw_circuit_connectors(
        &self,
        painter: &egui::Painter,
        metrics: &LayoutMetrics,
        colors: &Colors,
        circuit_origin: egui::Pos2,
        dragging_gate_id: Option<u32>,
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
            circuit_origin,
            dragging_gate_id,
        );
        swap::draw_swap_connectors(
            self,
            painter,
            metrics,
            colors,
            circuit_origin,
            dragging_gate_id,
        );
        phase::draw_phase_connectors_and_labels(
            self,
            painter,
            metrics,
            colors,
            circuit_origin,
            dragging_gate_id,
        );
    }
}
