use eframe::egui;

use crate::app::QniApp;
use crate::colors::Colors;
use crate::gates::GateKind;
use crate::icons::{draw_bloch_vector, draw_drag_gate_body};
use crate::layout::{amplitude_grid_rect, gate_visible_rect};

impl QniApp {
    pub(crate) fn draw_drag_preview(
        &self,
        painter: &egui::Painter,
        content_rect: egui::Rect,
        colors: &Colors,
        dragging_gate_id: u32,
        scroll_x: f32,
        live_drag_gpu_overlay_ready: bool,
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
        let body_rect = if gate.kind == GateKind::AmplitudeDisplay {
            amplitude_grid_rect(gate_rect, gate.span)
        } else {
            gate_rect
        };
        if live_drag_gpu_overlay_ready {
            // The snapped live display preview is rendered by the circuit's
            // single GPU callback. Do not add a second callback here: egui
            // prepares callbacks before painting them, and the callbacks use
            // shared instance buffers, so multiple callbacks in the same frame
            // overwrite each other's geometry.
            return;
        }
        draw_drag_gate_body(painter, body_rect, gate.kind, gate.span, colors);
        if gate.kind == GateKind::BlochDisplay {
            // While dragging the gate isn't snapped, so we can't compute a
            // Bloch vector. Render the qni d=0 blue dot at the sphere center.
            draw_bloch_vector(painter, gate_rect, [0.0, 0.0, 0.0], colors);
        }
    }

    pub(crate) fn live_drag_gpu_overlay_ready(&self, gate_id: u32) -> bool {
        if !self.dragging_live_display_snap {
            return false;
        }
        let Some(gate) = self.placed_gates.iter().find(|gate| gate.id == gate_id) else {
            return false;
        };
        match gate.kind {
            GateKind::AmplitudeDisplay => self.gpu_plan.amplitude_slot(gate.id).is_some(),
            GateKind::BlochDisplay => self.gpu_plan.bloch_slot(gate.id).is_some(),
            GateKind::Measurement => self.gpu_plan.has_measurement_slot(gate.id),
            _ => false,
        }
    }
}
