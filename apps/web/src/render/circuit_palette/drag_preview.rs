use eframe::{egui, egui_wgpu};

use crate::app::QniApp;
use crate::colors::Colors;
use crate::gates::GateKind;
use crate::gpu::{AmplitudeDisplayCallback, AmplitudeInstance, AMPLITUDE_FORCE_ZERO};
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
            amplitude_grid_rect(gate_rect, gate.span.get())
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
        if gate.kind == GateKind::AmplitudeDisplay && gate.span.get() == 1 {
            draw_zero_amplitude_drag_preview(painter, body_rect, colors);
            return;
        }
        draw_drag_gate_body(painter, body_rect, gate.kind, colors);
        if gate.kind == GateKind::BlochDisplay {
            // While dragging the gate isn't snapped, so we can't compute a
            // Bloch vector. Render the tx-3 center dot at the sphere center.
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
            GateKind::DensityMatrixDisplay => self.gpu_plan.density_slot(gate.id).is_some(),
            GateKind::BlochDisplay => self.gpu_plan.bloch_slot(gate.id).is_some(),
            GateKind::Measurement => self.gpu_plan.has_measurement_slot(gate.id),
            _ => false,
        }
    }
}

fn draw_zero_amplitude_drag_preview(
    painter: &egui::Painter,
    body_rect: egui::Rect,
    colors: &Colors,
) {
    let callback_rect = body_rect.intersect(painter.clip_rect());
    if callback_rect.width() <= 0.0 || callback_rect.height() <= 0.0 {
        return;
    }
    let callback = AmplitudeDisplayCallback {
        instances: vec![AmplitudeInstance {
            rect_min: [body_rect.min.x, body_rect.min.y],
            rect_size: [body_rect.width(), body_rect.height()],
            slot: 0,
            span: 1,
            hovered_outcome: -1,
            use_drag_background: 1,
            force_zero_amplitude: AMPLITUDE_FORCE_ZERO,
        }]
        .into(),
        use_drag_preview_buffer: true,
        viewport_min: [callback_rect.min.x, callback_rect.min.y],
        viewport_size: [callback_rect.width(), callback_rect.height()],
        background: colors.surface.to_normalized_gamma_f32(),
        drag_background: colors.drag_fill.to_normalized_gamma_f32(),
        border: colors.line.to_normalized_gamma_f32(),
        disk: colors.state_fill.to_normalized_gamma_f32(),
        disk_border: colors.amplitude_disk_border.to_normalized_gamma_f32(),
        outline: colors.state_outline.to_normalized_gamma_f32(),
        outline_zero: colors.state_outline_zero.to_normalized_gamma_f32(),
        needle: colors.state_needle.to_normalized_gamma_f32(),
        hover_border: colors.gate_hover_border.to_normalized_gamma_f32(),
        placeholder_background: colors.display_placeholder_fill.to_normalized_gamma_f32(),
        external_uploads: None,
    };
    let paint_callback = egui_wgpu::Callback::new_paint_callback(callback_rect, callback);
    painter.add(egui::Shape::Callback(paint_callback));
}
