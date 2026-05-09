use eframe::egui;
use eframe::{egui_wgpu, wgpu};
use std::cmp::Ordering;
use std::collections::HashMap;

use crate::app::{PlacedGate, QniApp};
use crate::colors::Colors;
use crate::constants::{
    state_circle_layout, CIRCUIT_PADDING, GATE_SIZE, LINE_GAP, LINE_Y, PALETTE_CORNER_RADIUS,
    PALETTE_GATES, PALETTE_PADDING_X, PALETTE_PADDING_Y, PALETTE_ROW_Y, PALETTE_SIZE, REM,
    SNAP_DISTANCE, STATE_CIRCLE_BOTTOM_MARGIN, STATE_HANDLE_HEIGHT,
};
use crate::gates::GateKind;
use crate::gpu::{
    BlochOverlayCallback, BlochOverlayInstance, MeasurementDigitCallback,
    MeasurementDigitInstance, RenderColors, StateVectorCallback,
};
use crate::icons::{draw_bloch_vector, draw_drag_gate_body, draw_gate_body, draw_meter_icon};
use crate::layout::{
    nearest_slot_index, palette_gate_local_pos, palette_layout, LayoutMetrics,
};
use crate::shared::amplitude_qubits;

pub(super) struct StatePanelLayout {
    state_count: usize,
    qubits: usize,
    columns: usize,
    size: f32,
    gap: f32,
    radius: f32,
    stroke: f32,
    inner_radius: f32,
    base_pos: egui::Pos2,
    pub(super) state_rect: egui::Rect,
    pub(super) handle_height: f32,
}

impl QniApp {
    pub(super) fn circuit_content_height(&self, qubit_count: usize, screen_height: f32) -> f32 {
        let line_count = qubit_count.max(1);
        let last_line_y = LINE_Y + LINE_GAP * (line_count.saturating_sub(1)) as f32;
        let content_height = last_line_y + GATE_SIZE + 4.0 * REM;
        content_height.max(screen_height)
    }

    pub(super) fn draw_circuit(
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

    pub(super) fn draw_palette(&self, painter: &egui::Painter, rect: egui::Rect, colors: &Colors) {
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

    pub(super) fn draw_drag_preview(
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

    pub(super) fn state_panel_layout(
        &self,
        rect: egui::Rect,
        state_count: usize,
    ) -> StatePanelLayout {
        let state_count = state_count.max(1);
        let qubits = amplitude_qubits(state_count);
        // qni reference: `apps/www/app/views/circuits/show.html.erb:66-67`
        // — `circle_notation` is rendered with `padding_x: 16, padding_y: 20`,
        // mirroring the palette's `px-4 py-5`. Reuse the palette constants.
        let state_padding_x = PALETTE_PADDING_X;
        let state_padding_y = PALETTE_PADDING_Y;

        // qni hard-codes the (cols, rows, size, line_width) per qubit count
        // (`circle-notation-element.ts:updateDimension/qubitCircleSizePx/qubitCircleLineWidth`).
        // Mirror that table so circles pack the same way: gap == stroke.
        let qni = state_circle_layout(qubits);
        let columns = qni.cols;
        let rows = qni.rows;
        let size = qni.size;
        let stroke = qni.line_width;
        let gap = qni.line_width;

        let total_width = size * columns as f32 + gap * (columns.saturating_sub(1)) as f32;
        let total_height = size * rows as f32 + gap * (rows.saturating_sub(1)) as f32;
        let base_y = rect.height() - STATE_CIRCLE_BOTTOM_MARGIN - total_height;
        let radius = size * 0.5;
        let inner_radius = (radius - stroke * 0.5).max(0.0);

        // qni-style header strip (G-2): fixed-height zinc-100 bar showing
        // qubit count + grid dims. Drag-to-move is the only interaction
        // attached to the strip for now (resize handles are TBD).
        let handle_height = STATE_HANDLE_HEIGHT;

        // Make sure the panel is wide enough that the strip's left/right
        // labels never overlap. Hack monospace at 11 px is ≈7 px / glyph;
        // budget a bit extra for the multiplication sign. The estimate is a
        // lower bound — for bigger circuits the circles already exceed it.
        const STRIP_CHAR_WIDTH: f32 = 7.0;
        const STRIP_PADDING_X: f32 = 12.0;
        const STRIP_LABEL_GAP: f32 = 16.0;
        let qubits_label = if qubits == 1 { "qubit" } else { "qubits" };
        let states_label = if state_count == 1 { "state" } else { "states" };
        let qubits_chars = format!("{qubits} {qubits_label}").chars().count();
        let states_chars = format!("{columns} × {rows} = {state_count} {states_label}")
            .chars()
            .count();
        let strip_min_width = (qubits_chars + states_chars) as f32 * STRIP_CHAR_WIDTH
            + STRIP_PADDING_X * 2.0
            + STRIP_LABEL_GAP;
        let panel_width = (total_width + state_padding_x * 2.0).max(strip_min_width);
        let panel_min_x = rect.width() / 2.0 - panel_width / 2.0;
        let base_pos = egui::pos2(
            rect.min.x + panel_min_x + (panel_width - total_width) / 2.0,
            rect.min.y + base_y,
        );
        let state_rect = egui::Rect::from_min_size(
            egui::pos2(
                rect.min.x + panel_min_x,
                base_pos.y - state_padding_y - handle_height,
            ),
            egui::vec2(
                panel_width,
                total_height + state_padding_y * 2.0 + handle_height,
            ),
        );

        StatePanelLayout {
            state_count,
            qubits,
            columns,
            size,
            gap,
            radius,
            stroke,
            inner_radius,
            base_pos,
            state_rect,
            handle_height,
        }
    }

    pub(super) fn clamp_state_panel_offset(&mut self, layout: &StatePanelLayout, rect: egui::Rect) {
        // The whole panel may extend past the screen edges (especially for
        // 16-qubit grids that are wider than the canvas), but the drag
        // handle must stay reachable — keep at least `MIN_VISIBLE` pixels
        // of it inside `rect` on both axes.
        const MIN_VISIBLE: f32 = 40.0;

        let panel_w = layout.state_rect.width();
        let handle_h = layout.handle_height;

        // Horizontal: panel right edge ≥ rect.min.x + MIN_VISIBLE  (left clip)
        //             panel left  edge ≤ rect.max.x − MIN_VISIBLE  (right clip)
        let min_x = rect.min.x + MIN_VISIBLE - panel_w;
        let max_x = rect.max.x - MIN_VISIBLE;
        // Vertical: handle bottom ≥ rect.min.y + MIN_VISIBLE   (top clip)
        //           handle top    ≤ rect.max.y − MIN_VISIBLE   (bottom clip)
        let min_y = rect.min.y + MIN_VISIBLE - handle_h;
        let max_y = rect.max.y - MIN_VISIBLE;

        let base_min = layout.state_rect.min;
        let min_offset_x = min_x - base_min.x;
        let max_offset_x = max_x - base_min.x;
        let min_offset_y = min_y - base_min.y;
        let max_offset_y = max_y - base_min.y;

        self.state_panel_offset.x = if max_offset_x < min_offset_x {
            min_offset_x
        } else {
            self.state_panel_offset.x.clamp(min_offset_x, max_offset_x)
        };
        self.state_panel_offset.y = if max_offset_y < min_offset_y {
            min_offset_y
        } else {
            self.state_panel_offset.y.clamp(min_offset_y, max_offset_y)
        };
    }


    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_state_vector(
        &mut self,
        painter: &egui::Painter,
        colors: &Colors,
        layout: &StatePanelLayout,
        offset: egui::Vec2,
        handle_height: f32,
        screen_rect: egui::Rect,
        recompute: bool,
        target_format: Option<wgpu::TextureFormat>,
    ) -> egui::Rect {
        let state_rect = layout.state_rect.translate(offset);
        let base_pos = layout.base_pos + offset;
        let state_corner = egui::CornerRadius::same(14);
        let state_shadow = egui::epaint::Shadow {
            offset: [0, 6],
            blur: 16,
            spread: 0,
            color: egui::Color32::from_rgba_unmultiplied(0, 0, 0, 25),
        };
        painter.add(egui::Shape::Rect(
            state_shadow.as_shape(state_rect, state_corner),
        ));
        painter.rect_filled(state_rect, state_corner, colors.surface);

        // G-2 header strip: zinc-100 bar with qubit count on the left and
        // "cols × rows = N states" on the right. Top corners follow the
        // panel's corner radius; the bottom edge is flat where the strip
        // meets the white panel body.
        let handle_rect = egui::Rect::from_min_size(
            state_rect.min,
            egui::vec2(state_rect.width(), handle_height.max(6.0)),
        );
        let handle_corner = egui::CornerRadius {
            nw: 14,
            ne: 14,
            sw: 0,
            se: 0,
        };
        painter.rect_filled(handle_rect, handle_corner, colors.state_handle_bg);

        let strip_padding_x = 12.0;
        let strip_font = egui::FontId::monospace(11.0);
        let qubits_label = if layout.qubits == 1 { "qubit" } else { "qubits" };
        let states_label = if layout.state_count == 1 { "state" } else { "states" };
        let qubits_text = format!("{} {}", layout.qubits, qubits_label);
        let rows = layout.state_count / layout.columns.max(1);
        let states_text = format!(
            "{} × {} = {} {}",
            layout.columns, rows, layout.state_count, states_label
        );
        // sky-500 strip → white text for legibility.
        painter.text(
            handle_rect.left_center() + egui::vec2(strip_padding_x, 0.0),
            egui::Align2::LEFT_CENTER,
            qubits_text,
            strip_font.clone(),
            colors.surface,
        );
        painter.text(
            handle_rect.right_center() - egui::vec2(strip_padding_x, 0.0),
            egui::Align2::RIGHT_CENTER,
            states_text,
            strip_font,
            colors.surface,
        );

        if let Some(target_format) = target_format {
            let sim_ops = if recompute {
                self.sim_ops.clone()
            } else {
                Vec::new()
            };
            let render_colors = RenderColors::new(colors);
            let callback_rect = screen_rect;
            let cell_pitch = layout.size + layout.gap;
            let cols = layout.columns as u32;
            let rows = (layout.state_count / layout.columns.max(1)) as u32;
            let render_params = crate::gpu::RenderParams {
                viewport_min: [callback_rect.min.x, callback_rect.min.y],
                viewport_size: [callback_rect.width(), callback_rect.height()],
                panel_origin: [base_pos.x, base_pos.y],
                panel_size: [cols as f32 * cell_pitch, rows as f32 * cell_pitch],
                cell_pitch,
                radius: layout.radius,
                inner_radius: layout.inner_radius,
                stroke: layout.stroke,
                cols,
                rows,
                qubits: layout.qubits as u32,
                _pad: 0,
                surface: render_colors.surface,
                fill: render_colors.fill,
                outline: render_colors.outline,
                outline_zero: render_colors.outline_zero,
                needle: render_colors.needle,
            };
            let callback = StateVectorCallback {
                sim_ops,
                state_count: layout.state_count,
                recompute,
                target_format,
                render_params,
            };
            let clipped = painter.with_clip_rect(state_rect);
            let paint_callback = egui_wgpu::Callback::new_paint_callback(callback_rect, callback);
            clipped.add(egui::Shape::Callback(paint_callback));
        }

        handle_rect
    }
}
