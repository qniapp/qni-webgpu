//! Placed circuit gates and GPU-backed circuit overlays.

use eframe::egui;
use eframe::egui_wgpu;

use crate::app::QniApp;
use crate::colors::Colors;
use crate::constants::{GATE_SIZE, LINE_GAP};
use crate::gates::GateKind;
use crate::gpu::{
    AmplitudeDisplayCallback, AmplitudeInstance, AmplitudePopupValueCallback, BlochOverlayCallback,
    BlochOverlayInstance, MeasurementDigitCallback, MeasurementDigitInstance,
    ProbabilityDisplayCallback, ProbabilityInstance, ProbabilityPopupValueCallback,
    POPUP_GLYPH_CELL_H, POPUP_GLYPH_CELL_W,
};
use crate::icons::{draw_bloch_vector, draw_gate_body, draw_meter_icon};
use crate::layout::{amplitude_grid_dims, amplitude_grid_rect, gate_visible_rect};
use crate::span_resize::{span_resize_body_rect, span_resize_ease_out_back, SpanResizeHandles};

// qni's Bloch vector tip is a 6px dot whose centre lands on the sphere
// circumference for ±Z states; the dot itself may extend slightly outside.
// Font-rasterisation baseline correction for the GPU atlas: keep the visible
// digit's vertical centre on the wire, matching qni's flex-centred value layer.
const MEASUREMENT_DIGIT_CENTER_Y_OFFSET: f32 = 1.0;
// Tailwind spacing-1 = 4px: qni shortens the measurement dropzone wires
// around the meter body, so the wire never touches or runs through the arc.
const MEASUREMENT_WIRE_CLEARANCE: f32 = 4.0;
fn probability_hover_popup_ket(outcome: u32, span: usize) -> String {
    let width = span.clamp(1, 16);
    format!("|{outcome:0width$b}⟩")
}

fn probability_hover_popup_subtitle(outcome: u32) -> String {
    format!("Probability if measured · k = {outcome}")
}

fn amplitude_hover_popup_ket(outcome: u32, span: usize) -> String {
    let width = span.clamp(1, 16);
    format!("|{outcome:0width$b}⟩")
}

fn amplitude_hover_popup_subtitle(outcome: u32) -> String {
    format!("Amplitude · k = {outcome}")
}

impl QniApp {
    fn amplitude_display_slot(&self, gate_id: u32) -> Option<u32> {
        self.external_gpu_amplitude_uploads
            .as_ref()
            .and_then(|batch| batch.slot_for_gate(gate_id))
            .or_else(|| self.gpu_plan.amplitude_slot(gate_id))
    }

    fn bloch_display_slot(&self, gate_id: u32) -> Option<u32> {
        self.external_gpu_bloch_uploads
            .as_ref()
            .and_then(|batch| batch.slot_for_gate(gate_id))
            .or_else(|| self.gpu_plan.bloch_slot(gate_id))
    }

    pub(super) fn draw_placed_circuit_gates(
        &self,
        painter: &egui::Painter,
        circuit_origin: egui::Pos2,
        colors: &Colors,
        fast_drag: bool,
        dragging_gate_id: Option<u32>,
    ) {
        for gate in &self.placed_gates {
            let live_dragging_gate = dragging_gate_id == Some(gate.id)
                && self.dragging_live_display_snap
                && ((gate.kind == GateKind::BlochDisplay
                    && self.bloch_display_slot(gate.id).is_some())
                    || (gate.kind == GateKind::Measurement
                        && self.gpu_plan.has_measurement_slot(gate.id)));
            if dragging_gate_id == Some(gate.id) && !live_dragging_gate {
                continue;
            }
            let gate_rect = gate_visible_rect(gate, circuit_origin + gate.pos.to_vec2());
            // docs/design-system/amplitude-display.html §04: Amplitude's footprint follows
            // slot spacing, but the visible matrix body is the square-cell
            // draw area. The shared span-resize component uses that same body.
            let body_rect = span_resize_body_rect(gate.kind, gate.span, gate_rect);
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
            let amplitude_cell_hovered = gate.kind == GateKind::AmplitudeDisplay
                && self
                    .hovered_amplitude_outcome
                    .is_some_and(|(id, _)| id == gate.id);
            if !fast_drag && self.hovered_gate_id == Some(gate.id) && !amplitude_cell_hovered {
                let hover_outer = body_rect.expand(4.0);
                // 接続線はゲート本体の下に描く。ホバー枠の内側を背景色で
                // 塗りつぶすと、Control / AntiControl / Swap / Phase などの
                // 透明なゲート内部を通る縦接続線まで消えてしまうため、
                // 回路上のホバーは全ゲートで内部を塗らない線だけのリングにする。
                painter.rect_stroke(
                    hover_outer,
                    egui::CornerRadius::same(10),
                    egui::Stroke::new(2.0, colors.gate_hover_border),
                    egui::StrokeKind::Inside,
                );
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
                draw_gate_body(painter, body_rect, gate.kind, colors);
            }
            // Resizable-span gates use one shared handle component: top +
            // bottom purple pills with hover/active scaling. Amplitude's
            // component body is its centred matrix, so width follows the body.
            if let Some(handles) = SpanResizeHandles::for_gate_at(gate, gate_rect) {
                let visible = self.hovered_gate_id == Some(gate.id)
                    || self.span_resize_drag.map(|d| d.gate_id) == Some(gate.id);
                let visible_t = painter.ctx().animate_bool_with_time_and_easing(
                    egui::Id::new(("span_resize_handles", gate.id)),
                    visible,
                    0.25,
                    span_resize_ease_out_back,
                );
                if visible || visible_t > 0.01 {
                    handles.paint(
                        painter,
                        colors,
                        self.hovered_span_resize_handle,
                        self.span_resize_drag,
                        visible_t,
                    );
                }
            }
            if gate.kind == GateKind::BlochDisplay && self.bloch_display_slot(gate.id).is_none() {
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

        // GPU overlay: draw Probability bars from the GPU-side
        // marginalization buffer. CPU supplies only geometry + hover row.
        let probability_instances: Vec<ProbabilityInstance> = self
            .placed_gates
            .iter()
            .filter_map(|gate| {
                if gate.kind != GateKind::ProbabilityDisplay {
                    return None;
                }
                if dragging_gate_id == Some(gate.id) {
                    return None;
                }
                let slot = self.gpu_plan.probability_slot(gate.id)?;
                let span = gate.span.clamp(1, 16) as u32;
                let gate_height = (span.saturating_sub(1)) as f32 * LINE_GAP + GATE_SIZE;
                let gate_rect = egui::Rect::from_min_size(
                    circuit_origin + gate.pos.to_vec2(),
                    egui::vec2(GATE_SIZE, gate_height),
                );
                let hovered_outcome = self
                    .hovered_probability_outcome
                    .filter(|(id, _)| *id == gate.id)
                    .map(|(_, outcome)| outcome as i32)
                    .unwrap_or(-1);
                Some(ProbabilityInstance {
                    rect_min: [gate_rect.min.x, gate_rect.min.y],
                    rect_size: [gate_rect.width(), gate_rect.height()],
                    slot,
                    span,
                    hovered_outcome,
                    _pad: 0,
                })
            })
            .collect();
        if !probability_instances.is_empty() {
            let callback = ProbabilityDisplayCallback {
                instances: probability_instances.into(),
                viewport_min: [callback_rect.min.x, callback_rect.min.y],
                viewport_size: [callback_rect.width(), callback_rect.height()],
                background: colors.surface.to_normalized_gamma_f32(),
                border: colors.line.to_normalized_gamma_f32(),
                log_hint: colors.probability_log_hint.to_normalized_gamma_f32(),
                bar: colors.state_fill.to_normalized_gamma_f32(),
                bar_edge: colors.popup_icon.to_normalized_gamma_f32(),
                hover_border: colors.gate_hover_border.to_normalized_gamma_f32(),
                text_color: colors.text_strong.to_normalized_gamma_f32(),
            };
            let paint_callback = egui_wgpu::Callback::new_paint_callback(callback_rect, callback);
            painter.add(egui::Shape::Callback(paint_callback));
        }

        let live_dragging_amplitude_id =
            dragging_gate_id.filter(|_| self.dragging_live_display_snap);
        let amplitude_instances: Vec<AmplitudeInstance> = self
            .placed_gates
            .iter()
            .filter_map(|gate| {
                if gate.kind != GateKind::AmplitudeDisplay {
                    return None;
                }
                let dragging_this_gate = dragging_gate_id == Some(gate.id);
                if dragging_this_gate && live_dragging_amplitude_id != Some(gate.id) {
                    return None;
                }
                let slot = self.amplitude_display_slot(gate.id)?;
                let span = gate.span.clamp(1, 16) as u32;
                let gate_rect = gate_visible_rect(gate, circuit_origin + gate.pos.to_vec2());
                let body_rect = amplitude_grid_rect(gate_rect, gate.span);
                let hovered_outcome = self
                    .hovered_amplitude_outcome
                    .filter(|(id, _)| *id == gate.id)
                    .map(|(_, outcome)| outcome as i32)
                    .unwrap_or(-1);
                Some(AmplitudeInstance {
                    rect_min: [body_rect.min.x, body_rect.min.y],
                    rect_size: [body_rect.width(), body_rect.height()],
                    slot,
                    span,
                    hovered_outcome,
                    use_drag_background: u32::from(dragging_this_gate),
                    force_zero_amplitude: 0,
                })
            })
            .collect();
        if !amplitude_instances.is_empty() {
            let callback = AmplitudeDisplayCallback {
                instances: amplitude_instances.into(),
                use_drag_preview_buffer: false,
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
                external_uploads: self.external_gpu_amplitude_uploads.clone(),
            };
            let paint_callback = egui_wgpu::Callback::new_paint_callback(callback_rect, callback);
            painter.add(egui::Shape::Callback(paint_callback));
        }

        // GPU overlay: draws the dynamic arrow + tip dot for every placed
        // BlochDisplay whose values are live in `bloch_output_buffer`. No
        // CPU readback — the fragment shader samples the storage buffer
        // directly.
        let live_dragging_bloch_id = dragging_gate_id.filter(|_| self.dragging_live_display_snap);
        let bloch_overlay_instances: Vec<BlochOverlayInstance> = self
            .placed_gates
            .iter()
            .filter_map(|gate| {
                if gate.kind != GateKind::BlochDisplay {
                    return None;
                }
                if dragging_gate_id == Some(gate.id) && live_dragging_bloch_id != Some(gate.id) {
                    return None;
                }
                let slot = self.bloch_display_slot(gate.id)?;
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
                external_uploads: self.external_gpu_bloch_uploads.clone(),
            };
            let paint_callback = egui_wgpu::Callback::new_paint_callback(callback_rect, callback);
            painter.add(egui::Shape::Callback(paint_callback));
        }

        // GPU overlay: 0/1 digit per measurement, sourced directly from
        // `measurement_aux_buffer.z`. Half extent is roughly digit-bounding
        // box + AA fringe.
        let live_dragging_measurement_id =
            dragging_gate_id.filter(|_| self.dragging_live_display_snap);
        let measurement_digit_instances: Vec<MeasurementDigitInstance> = self
            .placed_gates
            .iter()
            .filter_map(|gate| {
                if gate.kind != GateKind::Measurement {
                    return None;
                }
                if dragging_gate_id == Some(gate.id)
                    && live_dragging_measurement_id != Some(gate.id)
                {
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
    }

    pub(crate) fn draw_amplitude_hover_popup(
        &self,
        painter: &egui::Painter,
        screen_rect: egui::Rect,
        circuit_origin: egui::Pos2,
        dragging_gate_id: Option<u32>,
        colors: &Colors,
    ) {
        let Some((gate_id, outcome)) = self.hovered_amplitude_outcome else {
            return;
        };
        if dragging_gate_id == Some(gate_id) {
            return;
        }
        let Some(gate) = self
            .placed_gates
            .iter()
            .find(|gate| gate.id == gate_id && gate.kind == GateKind::AmplitudeDisplay)
        else {
            return;
        };
        let Some(slot) = self.amplitude_display_slot(gate.id) else {
            return;
        };
        let span = gate.span.clamp(1, 16);
        let gate_rect = gate_visible_rect(gate, circuit_origin + gate.pos.to_vec2());
        let grid_rect = amplitude_grid_rect(gate_rect, span);
        let (cols, rows) = amplitude_grid_dims(span);
        let cell = grid_rect.width() / cols as f32;
        let outcome_usize = outcome as usize;
        if outcome_usize >= cols * rows {
            return;
        }
        let row = outcome_usize / cols;
        let row_top = grid_rect.top() + row as f32 * cell;
        let ket = amplitude_hover_popup_ket(outcome, span);
        let subtitle = amplitude_hover_popup_subtitle(outcome);

        // Popup typography follows docs/design-system/amplitude-display.html §09:
        // text-sm ket, text-xs subtitle/labels, Tailwind 4px spacing scale.
        let ket_font = egui::FontId::monospace(14.0); // text-sm = 14px.
        let subtitle_font = egui::FontId::proportional(12.0); // text-xs = 12px.
        let label_font = egui::FontId::monospace(12.0); // text-xs = 12px.
        let ket_galley = painter.layout_no_wrap(ket, ket_font, colors.text_strong);
        let subtitle_galley = painter.layout_no_wrap(subtitle, subtitle_font, colors.text);
        let val_label_galley =
            painter.layout_no_wrap("VAL".to_owned(), label_font.clone(), colors.text);
        let mag_label_galley =
            painter.layout_no_wrap("MAG²".to_owned(), label_font.clone(), colors.text);
        let phase_label_galley =
            painter.layout_no_wrap("PHASE".to_owned(), label_font, colors.text);

        let pad_x = 16.0; // spacing-4.
        let pad_y = 12.0; // spacing-3.
        let subtitle_gap = 4.0; // spacing-1.
        let divider_gap = 12.0; // spacing-3.
        let row_gap = 16.0; // spacing-4.
        let value_chars = 18.0; // Matches WGSL CHARS_PER_ROW.
        let value_w = POPUP_GLYPH_CELL_W as f32 * value_chars;
        let value_h = POPUP_GLYPH_CELL_H as f32;
        let value_pitch = 20.0; // text-sm default line-height = 20px.
        let label_w = 44.0_f32
            .max(val_label_galley.size().x)
            .max(mag_label_galley.size().x)
            .max(phase_label_galley.size().x);
        let row_w = label_w + row_gap + value_w;
        let content_w = ket_galley.size().x.max(subtitle_galley.size().x).max(row_w);
        let width = content_w + pad_x * 2.0;
        let ket_h = ket_galley.size().y.max(16.0);
        let subtitle_h = subtitle_galley.size().y.max(16.0);
        let height = pad_y * 2.0
            + ket_h
            + subtitle_gap
            + subtitle_h
            + divider_gap
            + 1.0
            + divider_gap
            + value_h
            + 4.0
            + value_h
            + 4.0
            + value_h;
        let rect =
            probability_hover_popup_rect(gate_rect, row_top, cell, width, height, screen_rect);
        let corner = egui::CornerRadius::same(6);
        let shadow = egui::epaint::Shadow {
            offset: [0, 4],
            blur: 12,
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

        let ket_pos = egui::pos2(
            rect.center().x - ket_galley.size().x * 0.5,
            rect.top() + pad_y,
        );
        let subtitle_pos = egui::pos2(rect.left() + pad_x, ket_pos.y + ket_h + subtitle_gap);
        let divider_y = subtitle_pos.y + subtitle_h + divider_gap;
        let row0_y = divider_y + 1.0 + divider_gap;
        let row1_y = row0_y + value_pitch;
        let row2_y = row1_y + value_pitch;
        painter.galley(ket_pos, ket_galley, colors.text_strong);
        painter.galley(subtitle_pos, subtitle_galley, colors.text);
        painter.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(rect.left() + pad_x, divider_y),
                egui::vec2(rect.width() - pad_x * 2.0, 1.0),
            ),
            egui::CornerRadius::ZERO,
            colors.line,
        );

        let label_x = rect.left() + pad_x;
        let value_x = rect.right() - pad_x - value_w;
        painter.galley(
            egui::pos2(label_x, row0_y + 2.0),
            val_label_galley,
            colors.text,
        );
        painter.galley(
            egui::pos2(label_x, row1_y + 2.0),
            mag_label_galley,
            colors.text,
        );
        painter.galley(
            egui::pos2(label_x, row2_y + 2.0),
            phase_label_galley,
            colors.text,
        );

        let value_anchor = egui::pos2(value_x, row0_y + 2.0);
        let callback = AmplitudePopupValueCallback {
            viewport_min: [screen_rect.min.x, screen_rect.min.y],
            viewport_size: [screen_rect.width(), screen_rect.height()],
            value_anchor: [value_anchor.x, value_anchor.y],
            row_pitch: value_pitch,
            char_size: [POPUP_GLYPH_CELL_W as f32, value_h],
            text_color: colors.text_strong.to_normalized_gamma_f32(),
            slot,
            outcome,
        };
        let paint_callback = egui_wgpu::Callback::new_paint_callback(screen_rect, callback);
        let value_clip = egui::Rect::from_min_size(
            value_anchor,
            egui::vec2(value_w, value_pitch * 2.0 + value_h),
        );
        painter
            .with_clip_rect(value_clip.intersect(rect.shrink(4.0)))
            .add(egui::Shape::Callback(paint_callback));
    }

    pub(crate) fn draw_probability_hover_popup(
        &self,
        painter: &egui::Painter,
        screen_rect: egui::Rect,
        circuit_origin: egui::Pos2,
        dragging_gate_id: Option<u32>,
        colors: &Colors,
    ) {
        let Some((gate_id, outcome)) = self.hovered_probability_outcome else {
            return;
        };
        if dragging_gate_id == Some(gate_id) {
            return;
        }
        let Some(gate) = self
            .placed_gates
            .iter()
            .find(|gate| gate.id == gate_id && gate.kind == GateKind::ProbabilityDisplay)
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
        let ket = probability_hover_popup_ket(outcome, span);
        let subtitle = probability_hover_popup_subtitle(outcome);
        let Some(slot) = self.gpu_plan.probability_slot(gate.id) else {
            return;
        };

        // Popup typography follows docs/design-system/probability-display.html §10: text-sm ket,
        // text-xs subtitle/labels, spacing on the Tailwind 4px scale.
        let ket_font = egui::FontId::monospace(14.0); // text-sm = 14px.
        let subtitle_font = egui::FontId::proportional(12.0); // text-xs = 12px.
        let label_font = egui::FontId::monospace(12.0); // text-xs = 12px.
        let ket_galley = painter.layout_no_wrap(ket, ket_font, colors.text_strong);
        let subtitle_galley = painter.layout_no_wrap(subtitle, subtitle_font, colors.text);
        let raw_label_galley =
            painter.layout_no_wrap("RAW".to_owned(), label_font.clone(), colors.text);
        let log_label_galley = painter.layout_no_wrap("LOG".to_owned(), label_font, colors.text);

        let pad_x = 16.0; // spacing-4.
        let pad_y = 12.0; // spacing-3.
        let subtitle_gap = 4.0; // spacing-1.
        let divider_gap = 12.0; // spacing-3.
        let row_gap = 16.0; // spacing-4.
        let value_chars = 12.0; // Must match WGSL VALUE_CHARS; fits raw percent and compact log dB rows.
        let value_w = POPUP_GLYPH_CELL_W as f32 * value_chars;
        let value_h = POPUP_GLYPH_CELL_H as f32;
        let value_pitch = 20.0; // text-sm default line-height = 20px.
        let label_w = 32.0_f32.max(raw_label_galley.size().x.max(log_label_galley.size().x));
        let row_w = label_w + row_gap + value_w;
        let content_w = ket_galley.size().x.max(subtitle_galley.size().x).max(row_w);
        let width = content_w + pad_x * 2.0;
        let ket_h = ket_galley.size().y.max(16.0);
        let subtitle_h = subtitle_galley.size().y.max(16.0);
        let height = pad_y * 2.0
            + ket_h
            + subtitle_gap
            + subtitle_h
            + divider_gap
            + 1.0
            + divider_gap
            + value_h
            + 4.0
            + value_h;
        let rect =
            probability_hover_popup_rect(gate_rect, row_top, row_h, width, height, screen_rect);
        let corner = egui::CornerRadius::same(6);
        let shadow = egui::epaint::Shadow {
            offset: [0, 4],
            blur: 12,
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

        let ket_pos = egui::pos2(
            rect.center().x - ket_galley.size().x * 0.5,
            rect.top() + pad_y,
        );
        let subtitle_pos = egui::pos2(rect.left() + pad_x, ket_pos.y + ket_h + subtitle_gap);
        let divider_y = subtitle_pos.y + subtitle_h + divider_gap;
        let raw_y = divider_y + 1.0 + divider_gap;
        let log_y = raw_y + value_pitch;
        painter.galley(ket_pos, ket_galley, colors.text_strong);
        painter.galley(subtitle_pos, subtitle_galley, colors.text);
        painter.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(rect.left() + pad_x, divider_y),
                egui::vec2(rect.width() - pad_x * 2.0, 1.0),
            ),
            egui::CornerRadius::ZERO,
            colors.line,
        );

        let label_x = rect.left() + pad_x;
        let value_x = rect.right() - pad_x - value_w;
        painter.galley(
            egui::pos2(label_x, raw_y + 2.0),
            raw_label_galley,
            colors.text,
        );
        painter.galley(
            egui::pos2(label_x, log_y + 2.0),
            log_label_galley,
            colors.text,
        );

        // Raw/log values come from the GPU Probability buffer via a
        // dedicated render pass. CPU paints only static labels; no readback.
        // Align the GPU-rasterised 16px-tall value cells with the
        // egui-painted RAW / LOG labels in the same popup rows.
        let value_anchor = egui::pos2(value_x, raw_y + 2.0);
        let value_color = colors.text_strong.to_normalized_gamma_f32();
        let callback = ProbabilityPopupValueCallback {
            viewport_min: [screen_rect.min.x, screen_rect.min.y],
            viewport_size: [screen_rect.width(), screen_rect.height()],
            value_anchor: [value_anchor.x, value_anchor.y],
            row_pitch: value_pitch,
            char_size: [POPUP_GLYPH_CELL_W as f32, value_h],
            text_color: value_color,
            slot,
            outcome,
        };
        let paint_callback = egui_wgpu::Callback::new_paint_callback(screen_rect, callback);
        let value_clip =
            egui::Rect::from_min_size(value_anchor, egui::vec2(value_w, value_pitch + value_h));
        painter
            .with_clip_rect(value_clip.intersect(rect.shrink(4.0)))
            .add(egui::Shape::Callback(paint_callback));
    }
}

fn probability_hover_popup_rect(
    gate_rect: egui::Rect,
    row_top: f32,
    row_h: f32,
    width: f32,
    height: f32,
    screen_rect: egui::Rect,
) -> egui::Rect {
    let side_gap = 8.0; // spacing-2.
    let size = egui::vec2(width, height);
    let y_offset = -height * 0.5 + row_h * 0.5;
    let right_rect =
        egui::Rect::from_min_size(egui::pos2(gate_rect.right() + side_gap, row_top), size)
            .translate(egui::vec2(0.0, y_offset));
    let mut rect = if right_rect.right() <= screen_rect.right() {
        right_rect
    } else {
        egui::Rect::from_min_size(
            egui::pos2(gate_rect.left() - side_gap - width, row_top),
            size,
        )
        .translate(egui::vec2(0.0, y_offset))
    };

    let viewport_pad = 8.0; // spacing-2.
    let min_left = screen_rect.left() + viewport_pad;
    let max_left = screen_rect.right() - viewport_pad - rect.width();
    let target_left = if max_left >= min_left {
        rect.left().clamp(min_left, max_left)
    } else {
        min_left
    };
    rect = rect.translate(egui::vec2(target_left - rect.left(), 0.0));
    rect
}

#[cfg(test)]
mod tests {
    use eframe::egui;

    #[test]
    fn probability_hover_popup_text_matches_probability_display_spec() {
        assert_eq!(
            (
                super::probability_hover_popup_ket(3, 2),
                super::probability_hover_popup_subtitle(3)
            ),
            (
                "|11⟩".to_owned(),
                "Probability if measured · k = 3".to_owned()
            )
        );
    }

    #[test]
    fn probability_hover_popup_flips_left_near_viewport_right_edge() {
        let gate_rect = egui::Rect::from_min_size(egui::pos2(200.0, 40.0), egui::vec2(40.0, 96.0));
        let screen_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(260.0, 180.0));

        let rect =
            super::probability_hover_popup_rect(gate_rect, 64.0, 24.0, 80.0, 60.0, screen_rect);

        assert_eq!((rect.left(), rect.right()), (112.0, 192.0));
    }

    #[test]
    fn probability_hover_popup_clamps_inside_viewport_left_edge() {
        let gate_rect = egui::Rect::from_min_size(egui::pos2(64.0, 40.0), egui::vec2(40.0, 96.0));
        let screen_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(120.0, 180.0));

        let rect =
            super::probability_hover_popup_rect(gate_rect, 64.0, 24.0, 140.0, 60.0, screen_rect);

        assert_eq!(rect.left(), 8.0);
    }
}
