//! Circuit area drawing — qubit lines, placed gates, palette, drag
//! preview. Independent of the state-vector panel.

use eframe::egui;
use eframe::egui_wgpu;
use std::cmp::Ordering;
use std::collections::HashMap;

use crate::app::{PlacedGate, QniApp};
use crate::colors::Colors;
use crate::constants::{
    CIRCUIT_PADDING, GATE_SIZE, LINE_GAP, LINE_Y, PALETTE_CORNER_RADIUS, PALETTE_GATES,
    PALETTE_PADDING_X, PALETTE_PADDING_Y, PALETTE_ROW_Y, PALETTE_SIZE, REM, SNAP_DISTANCE,
};
use crate::gates::GateKind;
use crate::gpu::{
    BlochOverlayCallback, BlochOverlayInstance, MeasurementDigitCallback, MeasurementDigitInstance,
};
use crate::icons::{draw_bloch_vector, draw_drag_gate_body, draw_gate_body, draw_meter_icon};
use crate::layout::{nearest_slot_index, palette_gate_local_pos, palette_layout, LayoutMetrics};

impl QniApp {
    pub(crate) fn circuit_content_height(&self, qubit_count: usize, screen_height: f32) -> f32 {
        let line_count = qubit_count.max(1);
        let last_line_y = LINE_Y + LINE_GAP * (line_count.saturating_sub(1)) as f32;
        let content_height = last_line_y + GATE_SIZE + 4.0 * REM;
        content_height.max(screen_height)
    }

    pub(crate) fn draw_circuit(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        metrics: &LayoutMetrics,
        colors: &Colors,
        fast_drag: bool,
        dragging_gate_id: Option<u32>,
    ) {
        for &line_y in &metrics.line_ys {
            let start = rect.min + egui::vec2(metrics.line_left, line_y);
            let end = rect.min + egui::vec2(metrics.line_right, line_y);
            painter.line_segment([start, end], egui::Stroke::new(2.0, colors.line));
        }

        if !fast_drag {
            let mut control_groups: HashMap<usize, (Vec<egui::Pos2>, Vec<egui::Pos2>)> =
                HashMap::new();
            for gate in &self.placed_gates {
                if gate.kind == GateKind::Swap {
                    continue;
                }
                let is_control =
                    gate.kind == GateKind::Control || gate.kind == GateKind::AntiControl;
                let center_x = gate.pos.x + GATE_SIZE / 2.0;
                if let Some((slot_index, distance)) =
                    nearest_slot_index(center_x, &metrics.slot_centers)
                {
                    if distance > SNAP_DISTANCE {
                        continue;
                    }
                    let center = rect.min
                        + gate.pos.to_vec2()
                        + egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0);
                    let entry = control_groups
                        .entry(slot_index)
                        .or_insert((Vec::new(), Vec::new()));
                    if is_control {
                        entry.0.push(center);
                    } else {
                        entry.1.push(center);
                    }
                }
            }

            for (_, (controls, targets)) in control_groups {
                if controls.is_empty() || targets.is_empty() {
                    continue;
                }
                let mut min_y = f32::INFINITY;
                let mut max_y = f32::NEG_INFINITY;
                let mut xs = Vec::with_capacity(controls.len() + targets.len());
                for point in controls.iter().chain(targets.iter()) {
                    min_y = min_y.min(point.y);
                    max_y = max_y.max(point.y);
                    xs.push(point.x);
                }
                let x = if xs.is_empty() {
                    continue;
                } else {
                    xs.iter().sum::<f32>() / xs.len() as f32
                };
                let stroke = egui::Stroke::new(GATE_SIZE / 12.0, colors.box_fill);
                painter.line_segment([egui::pos2(x, min_y), egui::pos2(x, max_y)], stroke);
            }

            let mut swap_groups: HashMap<usize, Vec<&PlacedGate>> = HashMap::new();
            for gate in &self.placed_gates {
                if gate.kind != GateKind::Swap {
                    continue;
                }
                let center_x = gate.pos.x + GATE_SIZE / 2.0;
                if let Some((slot_index, distance)) =
                    nearest_slot_index(center_x, &metrics.slot_centers)
                {
                    if distance <= SNAP_DISTANCE {
                        swap_groups.entry(slot_index).or_default().push(gate);
                    }
                }
            }

            for (_, gates) in swap_groups {
                if gates.len() < 2 {
                    continue;
                }
                let mut centers = gates
                    .iter()
                    .map(|gate| {
                        rect.min + gate.pos.to_vec2() + egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0)
                    })
                    .collect::<Vec<_>>();
                centers.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap_or(Ordering::Equal));
                let top = centers.first().copied();
                let bottom = centers.last().copied();
                if let (Some(top), Some(bottom)) = (top, bottom) {
                    let swap_stroke = egui::Stroke::new(GATE_SIZE / 12.0, colors.box_fill);
                    painter.line_segment([top, bottom], swap_stroke);
                }
            }
        }

        for gate in &self.placed_gates {
            if dragging_gate_id == Some(gate.id) {
                continue;
            }
            let gate_rect = egui::Rect::from_min_size(
                rect.min + gate.pos.to_vec2(),
                egui::vec2(GATE_SIZE, GATE_SIZE),
            );
            if !fast_drag && self.hovered_gate_id == Some(gate.id) {
                let hover_outer = gate_rect.expand(4.0);
                let hover_inner = gate_rect.expand(2.0);
                painter.rect_filled(hover_outer, egui::CornerRadius::same(10), colors.box_border);
                painter.rect_filled(hover_inner, egui::CornerRadius::same(8), colors.background);
            } else if matches!(gate.kind, GateKind::Write0 | GateKind::Write1) {
                // Write gates have no fill, so the wire would otherwise show
                // through the brackets. Mask just the wire under the gate.
                painter.rect_filled(gate_rect, egui::CornerRadius::ZERO, colors.background);
            }
            draw_gate_body(painter, gate_rect, gate.kind, colors);
            if gate.kind == GateKind::Measurement && self.measurement_slots.contains_key(&gate.id) {
                // Repaint the meter in zinc-200 ("fired" appearance per qni's
                // `measurement_gate.css`). The GPU `MeasurementDigitCallback`
                // overlays the colored 0/1 digit directly from
                // `measurement_aux_buffer.z` — no CPU readback.
                draw_meter_icon(painter, gate_rect, colors.measurement_fired_icon);
            }
            if gate.kind == GateKind::BlochDisplay && !self.bloch_slots.contains_key(&gate.id) {
                // Not yet captured by a recompute (placed mid-drag, unsnapped,
                // or before the first frame's GPU dispatch). Show qni's
                // d=0 blue dot via egui until the GPU overlay takes over.
                draw_bloch_vector(painter, gate_rect, [0.0, 0.0, 0.0], colors);
            }
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
                let slot = *self.bloch_slots.get(&gate.id)?;
                let gate_rect = egui::Rect::from_min_size(
                    rect.min + gate.pos.to_vec2(),
                    egui::vec2(GATE_SIZE, GATE_SIZE),
                );
                let center = gate_rect.center();
                let radius = gate_rect.width().min(gate_rect.height()) * 0.5 - 1.0;
                // 4px slack covers the 3px tip dot + 1px AA fringe.
                let outer = radius + 4.0;
                Some(BlochOverlayInstance {
                    center: [center.x, center.y],
                    radius,
                    outer,
                    slot,
                })
            })
            .collect();
        if !bloch_overlay_instances.is_empty() {
            let callback = BlochOverlayCallback {
                instances: bloch_overlay_instances.into(),
                viewport_min: [rect.min.x, rect.min.y],
                viewport_size: [rect.width(), rect.height()],
                // Same gamma story as `RenderColors::new` — surface is
                // rgba8unorm so we hand the GPU sRGB bytes, not linear.
                line_color: colors.bloch_vector_line.to_normalized_gamma_f32(),
                tip_color: colors.bloch_vector_tip.to_normalized_gamma_f32(),
                zero_color: colors.bloch_vector_zero.to_normalized_gamma_f32(),
            };
            let paint_callback = egui_wgpu::Callback::new_paint_callback(rect, callback);
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
                let slot = *self.measurement_slots.get(&gate.id)?;
                let gate_rect = egui::Rect::from_min_size(
                    rect.min + gate.pos.to_vec2(),
                    egui::vec2(GATE_SIZE, GATE_SIZE),
                );
                let center = gate_rect.center();
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
                viewport_min: [rect.min.x, rect.min.y],
                viewport_size: [rect.width(), rect.height()],
                // Surface is rgba8unorm (non-sRGB) — egui paints text using
                // sRGB-encoded colours straight to the framebuffer, so we
                // need to do the same for the digit to read identically.
                // `Rgba::from(Color32)` would convert to linear and produce
                // a desaturated digit.
                zero_color: colors.semantic_off.to_normalized_gamma_f32(),
                one_color: colors.semantic_on.to_normalized_gamma_f32(),
            };
            let paint_callback = egui_wgpu::Callback::new_paint_callback(rect, callback);
            painter.add(egui::Shape::Callback(paint_callback));
        }

        for (index, &line_y) in metrics.line_ys.iter().enumerate() {
            let label_pos = rect.min + egui::vec2(CIRCUIT_PADDING, line_y - 7.0);
            painter.text(
                label_pos,
                egui::Align2::LEFT_TOP,
                format!("q{index}:"),
                egui::FontId::proportional(14.0),
                colors.text,
            );
        }
    }

    pub(crate) fn draw_palette(&self, painter: &egui::Painter, rect: egui::Rect, colors: &Colors) {
        let layout = palette_layout();
        let palette_start_x = rect.width() / 2.0 - layout.total_width / 2.0;
        let palette_rect = egui::Rect::from_min_size(
            rect.min
                + egui::vec2(
                    palette_start_x - PALETTE_PADDING_X,
                    PALETTE_ROW_Y - PALETTE_PADDING_Y,
                ),
            egui::vec2(
                layout.total_width + PALETTE_PADDING_X * 2.0,
                layout.total_height + PALETTE_PADDING_Y * 2.0,
            ),
        );
        let palette_corner = egui::CornerRadius::same(PALETTE_CORNER_RADIUS);
        let shadow = egui::epaint::Shadow {
            offset: [0, 6],
            blur: 16,
            spread: 0,
            color: egui::Color32::from_rgba_unmultiplied(0, 0, 0, 25),
        };
        painter.add(egui::Shape::Rect(
            shadow.as_shape(palette_rect, palette_corner),
        ));
        painter.rect_filled(palette_rect, palette_corner, colors.surface);

        let palette_origin = rect.min + egui::vec2(palette_start_x, PALETTE_ROW_Y);
        for (index, gate) in PALETTE_GATES.iter().enumerate() {
            let Some(local) = palette_gate_local_pos(index, &layout) else {
                continue;
            };
            let gate_rect = egui::Rect::from_min_size(
                palette_origin + local.to_vec2(),
                egui::vec2(PALETTE_SIZE, PALETTE_SIZE),
            );
            if self.hovered_palette_index == Some(index) {
                let hover_outer = gate_rect.expand(4.0);
                let hover_inner = gate_rect.expand(2.0);
                painter.rect_filled(hover_outer, egui::CornerRadius::same(10), colors.box_border);
                painter.rect_filled(hover_inner, egui::CornerRadius::same(8), colors.background);
            }
            draw_gate_body(painter, gate_rect, *gate, colors);
            if *gate == GateKind::BlochDisplay {
                // Palette has no associated state: render qni's d=0 blue center dot.
                draw_bloch_vector(painter, gate_rect, [0.0, 0.0, 0.0], colors);
            }
        }
    }

    pub(crate) fn draw_drag_preview(
        &self,
        painter: &egui::Painter,
        content_rect: egui::Rect,
        colors: &Colors,
        dragging_gate_id: u32,
    ) {
        let Some(gate) = self.placed_gates.iter().find(|gate| gate.id == dragging_gate_id) else {
            return;
        };
        let gate_rect = egui::Rect::from_min_size(
            content_rect.min + gate.pos.to_vec2(),
            egui::vec2(GATE_SIZE, GATE_SIZE),
        );
        draw_drag_gate_body(painter, gate_rect, gate.kind, colors);
        if gate.kind == GateKind::BlochDisplay {
            // While dragging the gate isn't snapped, so we can't compute a
            // Bloch vector. Render the qni d=0 blue dot at the sphere center.
            draw_bloch_vector(painter, gate_rect, [0.0, 0.0, 0.0], colors);
        }
    }
}
