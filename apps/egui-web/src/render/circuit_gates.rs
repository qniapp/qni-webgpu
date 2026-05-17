//! Placed circuit gates and GPU-backed circuit overlays.

use eframe::egui;
use eframe::egui_wgpu;

use crate::app::QniApp;
use crate::colors::Colors;
use crate::constants::{GATE_SIZE, LINE_GAP};
use crate::gates::GateKind;
use crate::gpu::{
    BlochOverlayCallback, BlochOverlayInstance, ChanceDisplayCallback, ChanceInstance,
    MeasurementDigitCallback, MeasurementDigitInstance,
};
use crate::icons::{
    draw_bloch_vector, draw_chance_resize_handle, draw_gate_body, draw_meter_icon,
    draw_qft_resize_handle,
};
use crate::layout::span_resize_handle_rect;

// qni's Bloch vector tip is a 6px dot whose centre lands on the sphere
// circumference for ±Z states; the dot itself may extend slightly outside.
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
            // Resizable-span gates are multi-qubit bodies — they extend
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
            let measurement_has_slot =
                gate.kind == GateKind::Measurement && self.gpu_plan.has_measurement_slot(gate.id);
            let circuit_fill = colors.background;
            if gate.kind == GateKind::Measurement {
                // qni shortens the input/output wire around a measurement
                // dropzone and the meter's interior is opaque. Mask the
                // circuit wire before painting hover chrome / SVG strokes /
                // digit overlay so the hover side borders remain visible.
                let mask_rect = gate_rect.expand2(egui::vec2(MEASUREMENT_WIRE_CLEARANCE, 0.0));
                painter.rect_filled(mask_rect, egui::CornerRadius::ZERO, circuit_fill);
            }
            if !fast_drag && self.hovered_gate_id == Some(gate.id) {
                let hover_outer = gate_rect.expand(4.0);
                let hover_inner = gate_rect.expand(2.0);
                let hover_inner_fill = if gate.kind == GateKind::Measurement {
                    circuit_fill
                } else {
                    colors.background
                };
                painter.rect_filled(
                    hover_outer,
                    egui::CornerRadius::same(10),
                    colors.gate_hover_border,
                );
                painter.rect_filled(hover_inner, egui::CornerRadius::same(8), hover_inner_fill);
            }
            if matches!(gate.kind, GateKind::Write0 | GateKind::Write1) {
                // Write gates have no fill, so the wire would otherwise show
                // through the brackets. Mask just the wire under the gate.
                painter.rect_filled(gate_rect, egui::CornerRadius::ZERO, colors.background);
            }
            if measurement_has_slot {
                // Repaint the meter in the same neutral tone as the wire after
                // masking the wire gap. Draw it once (instead of purple then
                // neutral) so anti-aliased edges do not leak the palette colour.
                draw_meter_icon(painter, gate_rect, colors.measurement_fired_icon);
            } else {
                draw_gate_body(painter, gate_rect, gate.kind, colors);
            }
            // Resizable-span gates: the bottom-edge resize handle appears
            // on hover (or while actively being resized). Drawn on top of the body.
            if gate.kind.is_resizable_span()
                && (self.hovered_gate_id == Some(gate.id)
                    || self.span_resize_drag.map(|d| d.gate_id) == Some(gate.id))
            {
                let active = self.hovered_span_resize_handle == Some(gate.id)
                    || self.span_resize_drag.map(|d| d.gate_id) == Some(gate.id);
                let bg = if active {
                    colors.span_resize_handle_bg_hover
                } else {
                    colors.span_resize_handle_bg
                };
                let handle_rect = span_resize_handle_rect(gate.kind, gate_rect);
                if gate.kind == GateKind::ChanceDisplay {
                    draw_chance_resize_handle(painter, handle_rect, bg, active);
                } else {
                    draw_qft_resize_handle(painter, handle_rect, bg, colors.label);
                }
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

        // GPU overlay: draw Chance probability bars from the GPU-side
        // marginalization buffer. CPU supplies only geometry + hover row.
        let chance_instances: Vec<ChanceInstance> = self
            .placed_gates
            .iter()
            .filter_map(|gate| {
                if gate.kind != GateKind::ChanceDisplay {
                    return None;
                }
                if dragging_gate_id == Some(gate.id) {
                    return None;
                }
                let slot = self.gpu_plan.chance_slot(gate.id)?;
                let span = gate.span.clamp(1, 16) as u32;
                let gate_height = (span.saturating_sub(1)) as f32 * LINE_GAP + GATE_SIZE;
                let gate_rect = egui::Rect::from_min_size(
                    circuit_origin + gate.pos.to_vec2(),
                    egui::vec2(GATE_SIZE, gate_height),
                );
                let hovered_outcome = self
                    .hovered_chance_outcome
                    .filter(|(id, _)| *id == gate.id)
                    .map(|(_, outcome)| outcome as i32)
                    .unwrap_or(-1);
                Some(ChanceInstance {
                    rect_min: [gate_rect.min.x, gate_rect.min.y],
                    rect_size: [gate_rect.width(), gate_rect.height()],
                    slot,
                    span,
                    hovered_outcome,
                    _pad: 0,
                })
            })
            .collect();
        if !chance_instances.is_empty() {
            let callback = ChanceDisplayCallback {
                instances: chance_instances.into(),
                viewport_min: [callback_rect.min.x, callback_rect.min.y],
                viewport_size: [callback_rect.width(), callback_rect.height()],
                background: colors.surface.to_normalized_gamma_f32(),
                border: colors.line.to_normalized_gamma_f32(),
                bar: colors.state_fill.to_normalized_gamma_f32(),
                bar_hover: colors.semantic_on.to_normalized_gamma_f32(),
                text_color: colors.text_strong.to_normalized_gamma_f32(),
            };
            let paint_callback = egui_wgpu::Callback::new_paint_callback(callback_rect, callback);
            painter.add(egui::Shape::Callback(paint_callback));
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
                // 4px slack covers the 3px tip dot + 1px AA fringe.
                let outer = sphere_radius + 4.0;
                Some(BlochOverlayInstance {
                    center: [center.x, center.y],
                    radius: sphere_radius,
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
                    // Quad spans the full gate body, matching the Write gate digit
                    // rect so Measurement and Write share the same SVG/SDF digit size.
                    half_extent: GATE_SIZE * 0.5,
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

        self.draw_chance_hover_popup(painter, circuit_origin, dragging_gate_id, colors);
    }

    fn draw_chance_hover_popup(
        &self,
        painter: &egui::Painter,
        circuit_origin: egui::Pos2,
        dragging_gate_id: Option<u32>,
        colors: &Colors,
    ) {
        let Some((gate_id, outcome)) = self.hovered_chance_outcome else {
            return;
        };
        if dragging_gate_id == Some(gate_id) {
            return;
        }
        let Some(gate) = self
            .placed_gates
            .iter()
            .find(|gate| gate.id == gate_id && gate.kind == GateKind::ChanceDisplay)
        else {
            return;
        };
        let span = gate.span.clamp(1, 16);
        let row_count = 1usize << span;
        let gate_height = (span - 1) as f32 * LINE_GAP + GATE_SIZE;
        let gate_rect = egui::Rect::from_min_size(
            circuit_origin + gate.pos.to_vec2(),
            egui::vec2(GATE_SIZE, gate_height),
        );
        let row_h = gate_rect.height() / row_count as f32;
        let row_top = gate_rect.top() + row_h * outcome as f32;
        let binary = format!("{:0width$b}", outcome, width = span);
        let title = format!("Chance of |{binary}⟩");
        let subtitle = "GPU probability bar";
        // text-xs = 12px / line-height 16px; spacing-2 = 8px padding.
        let title_font = egui::FontId::proportional(12.0);
        let subtitle_font = egui::FontId::monospace(12.0);
        let title_galley = painter.layout_no_wrap(title, title_font, colors.text_strong);
        let subtitle_galley =
            painter.layout_no_wrap(subtitle.to_owned(), subtitle_font, colors.text);
        let width = title_galley.size().x.max(subtitle_galley.size().x) + 16.0;
        let height = 8.0 + 16.0 + 16.0 + 8.0;
        let mut rect = egui::Rect::from_min_size(
            egui::pos2(gate_rect.right() + 8.0, row_top),
            egui::vec2(width, height),
        );
        rect = rect.translate(egui::vec2(0.0, -rect.height() * 0.5 + row_h * 0.5));
        let corner = egui::CornerRadius::same(6);
        let shadow = egui::epaint::Shadow {
            offset: [0, 6],
            blur: 16,
            spread: 0,
            color: colors.tooltip_shadow,
        };
        painter.add(egui::Shape::Rect(shadow.as_shape(rect, corner)));
        painter.rect_filled(rect, corner, colors.surface);
        painter.rect_stroke(
            rect,
            corner,
            egui::Stroke::new(1.0, colors.line),
            egui::StrokeKind::Inside,
        );
        painter.galley(
            rect.min + egui::vec2(8.0, 7.0),
            title_galley,
            colors.text_strong,
        );
        painter.galley(
            rect.min + egui::vec2(8.0, 23.0),
            subtitle_galley,
            colors.text,
        );
    }
}
