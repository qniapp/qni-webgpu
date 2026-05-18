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

// Tailwind spacing-1 = 4px. Use the same even-width stroke for all vertical
// gate connectors (Control / Swap / same-angle Phase) so the line has no
// one-pixel left/right bias on the even-sized 40px gate grid.
pub(super) const CONNECTOR_STROKE_WIDTH: f32 = 4.0;
// The 40px gate centre lands on an integer canvas coordinate. Egui's line
// rasterization looks visually centred over the even-sized icons when vertical
// connectors are nudged by half a pixel.
pub(super) const CONNECTOR_VISUAL_X_OFFSET: f32 = -0.5;

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
