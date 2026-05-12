//! Circuit connector and phase-label drawing facade.
//!
//! Connector families are split by gate relationship: controlled columns,
//! swap pairs, and same-angle phase chains / labels.

mod control;
mod phase;
mod swap;

use eframe::egui;

use crate::app::QniApp;
use crate::colors::Colors;
use crate::layout::LayoutMetrics;

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
