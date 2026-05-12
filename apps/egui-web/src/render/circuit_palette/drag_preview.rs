use eframe::egui;

use crate::app::QniApp;
use crate::colors::Colors;
use crate::gates::GateKind;
use crate::icons::{draw_bloch_vector, draw_drag_gate_body};
use crate::layout::gate_visible_rect;

impl QniApp {
    pub(crate) fn draw_drag_preview(
        &self,
        painter: &egui::Painter,
        content_rect: egui::Rect,
        colors: &Colors,
        dragging_gate_id: u32,
        scroll_x: f32,
    ) {
        let Some(gate) = self
            .placed_gates
            .iter()
            .find(|gate| gate.id == dragging_gate_id)
        else {
            return;
        };
        // Same convention as draw_circuit — gate.pos is in circuit
        // space, so we shift the content_rect origin left by the scroll
        // offset before placing the drag preview.
        let circuit_origin = content_rect.min - egui::vec2(scroll_x, 0.0);
        let gate_rect = gate_visible_rect(gate, circuit_origin + gate.pos.to_vec2());
        draw_drag_gate_body(painter, gate_rect, gate.kind, colors);
        if gate.kind == GateKind::BlochDisplay {
            // While dragging the gate isn't snapped, so we can't compute a
            // Bloch vector. Render the qni d=0 blue dot at the sphere center.
            draw_bloch_vector(painter, gate_rect, [0.0, 0.0, 0.0], colors);
        }
    }
}
