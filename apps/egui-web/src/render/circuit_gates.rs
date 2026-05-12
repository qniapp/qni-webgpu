//! Placed circuit gates and GPU-backed circuit overlays.

use eframe::egui;
use eframe::egui_wgpu;

use crate::app::QniApp;
use crate::colors::Colors;
use crate::constants::{GATE_SIZE, LINE_GAP};
use crate::gates::GateKind;
use crate::gpu::{
    BlochOverlayCallback, BlochOverlayInstance, MeasurementDigitCallback, MeasurementDigitInstance,
};
use crate::icons::{draw_bloch_vector, draw_gate_body, draw_meter_icon, draw_qft_resize_handle};
use crate::layout::qft_resize_handle_rect;

// qni's Bloch vector tip is a 6px dot. Keep the dot's centre inset by its
// radius so the needle reads as attached to the sphere rather than floating
// outside the outline at ±Z.
const BLOCH_VECTOR_TIP_RADIUS: f32 = 3.0;
// Font-rasterisation baseline correction for the GPU atlas: keep the visible
// digit's vertical centre on the wire, matching qni's flex-centred value layer.
const MEASUREMENT_DIGIT_CENTER_Y_OFFSET: f32 = 1.0;
// Tailwind spacing-1 = 4px: qni shortens the measurement dropzone wires
// around the meter body, so the wire never touches or runs through the arc.
const MEASUREMENT_WIRE_CLEARANCE: f32 = 4.0;

impl QniApp {
    pub(super) fn draw_placed_circuit_gates(
        &self,
        painter: &egui::Painter,
        circuit_origin: egui::Pos2,
        colors: &Colors,
        fast_drag: bool,
        dragging_gate_id: Option<u32>,
    ) {
        for gate in &self.placed_gates {
            if dragging_gate_id == Some(gate.id) {
                continue;
            }
            // QFT family is a multi-qubit gate — its body extends
            // downward to cover `span` wires. Other gates stay single-
            // qubit (GATE_SIZE × GATE_SIZE).
            let gate_height = if gate.kind.is_resizable_span() {
                let span = gate.span.max(1);
                (span - 1) as f32 * LINE_GAP + GATE_SIZE
            } else {
                GATE_SIZE
            };
            let gate_rect = egui::Rect::from_min_size(
                circuit_origin + gate.pos.to_vec2(),
                egui::vec2(GATE_SIZE, gate_height),
            );
            if !fast_drag && self.hovered_gate_id == Some(gate.id) {
                let hover_outer = gate_rect.expand(4.0);
                let hover_inner = gate_rect.expand(2.0);
                painter.rect_filled(hover_outer, egui::CornerRadius::same(10), colors.box_border);
                painter.rect_filled(hover_inner, egui::CornerRadius::same(8), colors.background);
            }
            if matches!(gate.kind, GateKind::Write0 | GateKind::Write1) {
                // Write gates have no fill, so the wire would otherwise show
                // through the brackets. Mask just the wire under the gate.
                painter.rect_filled(gate_rect, egui::CornerRadius::ZERO, colors.background);
            }
            if gate.kind == GateKind::Measurement {
                // qni shortens the input/output wire around a measurement
                // dropzone and the meter's interior is opaque. Mask the
                // circuit wire before painting the SVG strokes/digit overlay.
                let mask_rect = gate_rect.expand2(egui::vec2(MEASUREMENT_WIRE_CLEARANCE, 0.0));
                let circuit_fill = painter.ctx().style().visuals.panel_fill;
                painter.rect_filled(mask_rect, egui::CornerRadius::ZERO, circuit_fill);
            }
            draw_gate_body(painter, gate_rect, gate.kind, colors);
            // QFT family: the bottom-edge resize handle appears on hover
            // (or while actively being resized). Drawn on top of the body.
            if gate.kind.is_resizable_span()
                && (self.hovered_gate_id == Some(gate.id)
                    || self.qft_resize_drag.map(|d| d.gate_id) == Some(gate.id))
            {
                let bg = if self.hovered_qft_resize_handle == Some(gate.id)
                    || self.qft_resize_drag.map(|d| d.gate_id) == Some(gate.id)
                {
                    colors.qft_resize_handle_bg_hover
                } else {
                    colors.qft_resize_handle_bg
                };
                let handle_rect = qft_resize_handle_rect(gate_rect);
                draw_qft_resize_handle(painter, handle_rect, bg);
            }
            if gate.kind == GateKind::Measurement && self.gpu_plan.has_measurement_slot(gate.id) {
                // Repaint the meter in zinc-200 ("fired" appearance per qni's
                // `measurement_gate.css`). The GPU `MeasurementDigitCallback`
                // overlays the colored 0/1 digit directly from
                // `measurement_aux_buffer.z` — no CPU readback.
                draw_meter_icon(painter, gate_rect, colors.measurement_fired_icon);
            }
            if gate.kind == GateKind::BlochDisplay && self.gpu_plan.bloch_slot(gate.id).is_none() {
                // Not yet captured by a recompute (placed mid-drag, unsnapped,
                // or before the first frame's GPU dispatch). Show qni's
                // d=0 blue dot via egui until the GPU overlay takes over.
                draw_bloch_vector(painter, gate_rect, [0.0, 0.0, 0.0], colors);
            }
        }
    }

    pub(super) fn draw_circuit_gpu_overlays(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        circuit_origin: egui::Pos2,
        dragging_gate_id: Option<u32>,
        colors: &Colors,
    ) {
        // Egui clamps callback viewports to the physical screen. Use the
        // visible clip intersection as the callback viewport; otherwise tall
        // circuits (e.g. 16 qubits) get vertically rescaled by wgpu and GPU
        // overlays drift above their egui-painted gate bodies.
        let callback_rect = rect.intersect(painter.clip_rect());
        if callback_rect.width() <= 0.0 || callback_rect.height() <= 0.0 {
            return;
        }

        // GPU overlay: draws the dynamic arrow + tip dot for every placed
        // BlochDisplay whose values are live in `bloch_output_buffer`. No
        // CPU readback — the fragment shader samples the storage buffer
        // directly.
        let bloch_overlay_instances: Vec<BlochOverlayInstance> = self
            .placed_gates
            .iter()
            .filter_map(|gate| {
                if gate.kind != GateKind::BlochDisplay {
                    return None;
                }
                if dragging_gate_id == Some(gate.id) {
                    return None;
                }
                let slot = self.gpu_plan.bloch_slot(gate.id)?;
                let gate_rect = egui::Rect::from_min_size(
                    circuit_origin + gate.pos.to_vec2(),
                    egui::vec2(GATE_SIZE, GATE_SIZE),
                );
                let center = gate_rect.center();
                let sphere_radius = gate_rect.width().min(gate_rect.height()) * 0.5 - 1.0;
                let vector_radius = (sphere_radius - BLOCH_VECTOR_TIP_RADIUS).max(0.0);
                // 4px slack covers the 3px tip dot + 1px AA fringe.
                let outer = sphere_radius + 4.0;
                Some(BlochOverlayInstance {
                    center: [center.x, center.y],
                    radius: vector_radius,
                    outer,
                    slot,
                })
            })
            .collect();
        if !bloch_overlay_instances.is_empty() {
            let callback = BlochOverlayCallback {
                instances: bloch_overlay_instances.into(),
                viewport_min: [callback_rect.min.x, callback_rect.min.y],
                viewport_size: [callback_rect.width(), callback_rect.height()],
                // Same gamma story as `RenderColors::new` — surface is
                // rgba8unorm so we hand the GPU sRGB bytes, not linear.
                line_color: colors.bloch_vector_line.to_normalized_gamma_f32(),
                tip_color: colors.bloch_vector_tip.to_normalized_gamma_f32(),
                zero_color: colors.bloch_vector_zero.to_normalized_gamma_f32(),
            };
            let paint_callback = egui_wgpu::Callback::new_paint_callback(callback_rect, callback);
            painter.add(egui::Shape::Callback(paint_callback));
        }

        // GPU overlay: 0/1 digit per measurement, sourced directly from
        // `measurement_aux_buffer.z`. Half extent is roughly digit-bounding
        // box + AA fringe.
        let measurement_digit_instances: Vec<MeasurementDigitInstance> = self
            .placed_gates
            .iter()
            .filter_map(|gate| {
                if gate.kind != GateKind::Measurement {
                    return None;
                }
                if dragging_gate_id == Some(gate.id) {
                    return None;
                }
                let slot = self.gpu_plan.measurement_slot(gate.id)?;
                let gate_rect = egui::Rect::from_min_size(
                    circuit_origin + gate.pos.to_vec2(),
                    egui::vec2(GATE_SIZE, GATE_SIZE),
                );
                let center =
                    gate_rect.center() + egui::vec2(0.0, MEASUREMENT_DIGIT_CENTER_Y_OFFSET);
                Some(MeasurementDigitInstance {
                    center: [center.x, center.y],
                    // Quad spans `2 * half_extent` px; matches the digit
                    // atlas cell size in `gpu.rs::DIGIT_ATLAS_CELL` so a
                    // glyph rasterised at the cell-pixel scale renders 1:1
                    // and matches egui's `FontId::monospace(16.0)`.
                    half_extent: 11.0,
                    slot,
                })
            })
            .collect();
        if !measurement_digit_instances.is_empty() {
            let callback = MeasurementDigitCallback {
                instances: measurement_digit_instances.into(),
                viewport_min: [callback_rect.min.x, callback_rect.min.y],
                viewport_size: [callback_rect.width(), callback_rect.height()],
                // Surface is rgba8unorm (non-sRGB) — egui paints text using
                // sRGB-encoded colours straight to the framebuffer, so we
                // need to do the same for the digit to read identically.
                // `Rgba::from(Color32)` would convert to linear and produce
                // a desaturated digit.
                zero_color: colors.semantic_off.to_normalized_gamma_f32(),
                one_color: colors.semantic_on.to_normalized_gamma_f32(),
            };
            let paint_callback = egui_wgpu::Callback::new_paint_callback(callback_rect, callback);
            painter.add(egui::Shape::Callback(paint_callback));
        }
    }
}
