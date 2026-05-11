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
use crate::icons::{
    draw_bloch_vector, draw_drag_gate_body, draw_gate_body, draw_meter_icon, draw_qft_resize_handle,
};
use crate::layout::{
    nearest_slot_index, palette_gate_local_pos, palette_layout, qft_resize_handle_rect,
    LayoutMetrics,
};

impl QniApp {
    pub(crate) fn circuit_content_height(&self, qubit_count: usize, screen_height: f32) -> f32 {
        let line_count = qubit_count.max(1);
        let last_line_y = LINE_Y + LINE_GAP * (line_count.saturating_sub(1)) as f32;
        let content_height = last_line_y + GATE_SIZE + 4.0 * REM;
        content_height.max(screen_height)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_circuit(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        metrics: &LayoutMetrics,
        colors: &Colors,
        fast_drag: bool,
        dragging_gate_id: Option<u32>,
        scroll_x: f32,
    ) {
        // `circuit_origin` is `rect.min` shifted left by the current
        // horizontal scroll offset. Anything pinned to the circuit's
        // coordinate system (wires, slot grid, gate bodies, step
        // indicators, connectors) is drawn relative to it; the qubit
        // label strip on the left and the GPU callback viewports stay
        // on `rect.min` so they don't track the scroll.
        let circuit_origin = rect.min - egui::vec2(scroll_x, 0.0);
        for &line_y in &metrics.line_ys {
            let start = circuit_origin + egui::vec2(metrics.line_left, line_y);
            let end = circuit_origin + egui::vec2(metrics.line_right, line_y);
            painter.line_segment([start, end], egui::Stroke::new(2.0, colors.line));
        }

        // Step-preview vertical bars at the right edge of the
        // hovered / breakpoint column. Hovered = 30% alpha (live
        // preview), breakpoint = full opacity (locked-in step). Mirrors
        // qni's `circuit-step::after` data-active / data-breakpoint
        // styling.
        if !metrics.line_ys.is_empty() && !metrics.slot_centers.is_empty() {
            let top = metrics.line_ys[0] - crate::constants::LINE_GAP * 0.5;
            let bot = metrics.line_ys[metrics.line_ys.len() - 1] + crate::constants::LINE_GAP * 0.5;
            let step_line = |painter: &egui::Painter, slot: usize, alpha: u8| {
                if slot >= metrics.slot_centers.len() {
                    return;
                }
                let x = metrics.slot_centers[slot]
                    + crate::constants::SLOT_SPACING * 0.5
                    + circuit_origin.x;
                // Flexoki blue-600 (#205EA6) — matches `state_fill`.
                let color = egui::Color32::from_rgba_unmultiplied(32, 94, 166, alpha);
                painter.line_segment(
                    [
                        egui::pos2(x, rect.min.y + top),
                        egui::pos2(x, rect.min.y + bot),
                    ],
                    egui::Stroke::new(3.0, color),
                );
            };
            if let Some(step) = self.breakpoint_step {
                step_line(painter, step, 255);
            }
            if let Some(step) = self.hovered_step {
                if Some(step) != self.breakpoint_step {
                    step_line(painter, step, 80);
                }
            }
        }

        // Connector lines (CNOT / CZ / Swap / Phase-Phase) are computed
        // every frame, including mid-drag, so a gate being moved into
        // a CNOT pair (or out of one) shows the line snapping live
        // instead of waiting for the drop. The work is cheap — one
        // pass over `placed_gates` per group — and well under the
        // dispatch budget at our 16-qubit cap.
        {
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
                    let center = circuit_origin
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

            for (slot_index, (controls, targets)) in &control_groups {
                // Connector is a *control-only* affordance: it tells
                // the reader "this column is a multi-qubit controlled
                // operation". Columns with no controls (e.g. four
                // parallel Hs, parallel Blochs, parallel writes) are
                // independent single-qubit gates and must NOT get a
                // line — matching qni's `circuit-step-element.ts:526`
                // early-return when both control lists are empty.
                if controls.is_empty() {
                    continue;
                }
                // A lone control with no controllable target is a
                // disabled no-op in qni (`:513-524`); we likewise skip
                // the line for it.
                if targets.is_empty() && controls.len() < 2 {
                    continue;
                }
                let mut min_y = f32::INFINITY;
                let mut max_y = f32::NEG_INFINITY;
                for point in controls.iter().chain(targets.iter()) {
                    min_y = min_y.min(point.y);
                    max_y = max_y.max(point.y);
                }
                // Anchor the line at the *slot center*, not the mean
                // of the gate centers — during a drag the moving gate
                // may sit a few pixels off the slot midpoint even
                // while it's inside `SNAP_DISTANCE`, and averaging
                // would pull the line off the column the snap is
                // actually going to commit to.
                let x = circuit_origin.x + metrics.slot_centers[*slot_index];
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

            for (slot_index, gates) in &swap_groups {
                if gates.len() < 2 {
                    continue;
                }
                let mut ys = gates
                    .iter()
                    .map(|gate| circuit_origin.y + gate.pos.y + GATE_SIZE / 2.0)
                    .collect::<Vec<_>>();
                ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
                let top_y = *ys.first().unwrap();
                let bottom_y = *ys.last().unwrap();
                // Slot-center anchored — same rationale as the control
                // connector above.
                let x = circuit_origin.x + metrics.slot_centers[*slot_index];
                let swap_stroke = egui::Stroke::new(GATE_SIZE / 12.0, colors.box_fill);
                painter.line_segment([egui::pos2(x, top_y), egui::pos2(x, bottom_y)], swap_stroke);
            }

            // Phase-Phase connector. qni's
            // `circuit-step-element.ts::updatePhasePhaseConnections`
            // (:566-602) draws a connector between same-angle Phase
            // gates in the same column. Semantically it's a *visual*
            // pairing only — qni's simulator still runs each Phase
            // independently (`simulator.ts::cu` :413-417 loops over
            // targets and applies the same 2x2 to each in turn), so we
            // mirror just the line rendering. Phases with no angle
            // (qni's empty placeholder) are skipped per :573.
            let mut phase_groups: HashMap<usize, HashMap<String, Vec<egui::Pos2>>> = HashMap::new();
            for gate in &self.placed_gates {
                if gate.kind != GateKind::Phase {
                    continue;
                }
                let angle = gate.angle.as_deref().unwrap_or("");
                if angle.is_empty() {
                    continue;
                }
                let center_x = gate.pos.x + GATE_SIZE / 2.0;
                if let Some((slot_index, distance)) =
                    nearest_slot_index(center_x, &metrics.slot_centers)
                {
                    if distance > SNAP_DISTANCE {
                        continue;
                    }
                    let center = circuit_origin
                        + gate.pos.to_vec2()
                        + egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0);
                    phase_groups
                        .entry(slot_index)
                        .or_default()
                        .entry(angle.to_string())
                        .or_default()
                        .push(center);
                }
            }
            for (slot_index, angle_buckets) in &phase_groups {
                for points in angle_buckets.values() {
                    if points.len() < 2 {
                        continue;
                    }
                    let mut min_y = f32::INFINITY;
                    let mut max_y = f32::NEG_INFINITY;
                    for point in points {
                        min_y = min_y.min(point.y);
                        max_y = max_y.max(point.y);
                    }
                    // Slot-center anchored — same rationale as the
                    // control / swap connectors above.
                    let x = circuit_origin.x + metrics.slot_centers[*slot_index];
                    let stroke = egui::Stroke::new(GATE_SIZE / 12.0, colors.box_fill);
                    painter.line_segment([egui::pos2(x, min_y), egui::pos2(x, max_y)], stroke);
                }
            }

            // Angle labels for Phase gates. qni puts the angle text just
            // outside the circular gate body (above for the topmost /
            // standalone gate in a same-angle pair, below for the
            // bottommost) so the label never overlaps the vertical
            // connector that ties same-angle Phase gates together. We
            // replicate the same dodge logic here.
            for gate in &self.placed_gates {
                if gate.kind != GateKind::Phase {
                    continue;
                }
                let Some(angle) = gate.angle.as_deref() else {
                    continue;
                };
                if angle.is_empty() {
                    continue;
                }
                let center_x = gate.pos.x + GATE_SIZE / 2.0;
                let Some((slot_index, distance)) =
                    nearest_slot_index(center_x, &metrics.slot_centers)
                else {
                    continue;
                };
                if distance > SNAP_DISTANCE {
                    continue;
                }
                let center = circuit_origin
                    + gate.pos.to_vec2()
                    + egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0);

                // Peers in the same column with the same angle.
                let mut peers_above = false;
                let mut peers_below = false;
                for other in &self.placed_gates {
                    if other.id == gate.id || other.kind != GateKind::Phase {
                        continue;
                    }
                    if other.angle.as_deref() != Some(angle) {
                        continue;
                    }
                    let other_center_x = other.pos.x + GATE_SIZE / 2.0;
                    let Some((other_slot, other_distance)) =
                        nearest_slot_index(other_center_x, &metrics.slot_centers)
                    else {
                        continue;
                    };
                    if other_slot != slot_index || other_distance > SNAP_DISTANCE {
                        continue;
                    }
                    if other.pos.y < gate.pos.y {
                        peers_above = true;
                    } else if other.pos.y > gate.pos.y {
                        peers_below = true;
                    }
                }

                // Above for the topmost / standalone gate; below for
                // the bottommost. A middle gate in a 3+ chain falls
                // back to above and is left to overlap the connector
                // (qni does the same).
                //   standalone (no peers)         → above
                //   topmost (peer below only)     → above
                //   bottommost (peer above only)  → below
                //   middle (peers above & below)  → above (fallback)
                let label_above = peers_below || !peers_above;
                let (label_y, align) = if label_above {
                    (
                        center.y - GATE_SIZE / 2.0 - 2.0,
                        egui::Align2::CENTER_BOTTOM,
                    )
                } else {
                    (center.y + GATE_SIZE / 2.0 + 2.0, egui::Align2::CENTER_TOP)
                };
                painter.text(
                    egui::pos2(center.x, label_y),
                    align,
                    angle,
                    // text-xs (12 px) — Tailwind. Matches the popup body
                    // font so labels feel like they belong to the same
                    // typographic system.
                    egui::FontId::monospace(12.0),
                    colors.text,
                );
            }
        }

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
            } else if matches!(gate.kind, GateKind::Write0 | GateKind::Write1) {
                // Write gates have no fill, so the wire would otherwise show
                // through the brackets. Mask just the wire under the gate.
                painter.rect_filled(gate_rect, egui::CornerRadius::ZERO, colors.background);
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
                    circuit_origin + gate.pos.to_vec2(),
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
                    circuit_origin + gate.pos.to_vec2(),
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
            // Labels live in circuit space (anchored to the wire's
            // start) so they scroll with the rest of the circuit —
            // otherwise the leftmost gates would slide under fixed
            // "q0:" / "q1:" labels and visually collide.
            let label_pos = circuit_origin + egui::vec2(CIRCUIT_PADDING, line_y - 7.0);
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

    /// Hover tooltip painted over the palette: a paper card with the
    /// gate's full name, qni-style description paragraphs, and a mini
    /// transformation diagram (input amplitudes → gate → output
    /// amplitudes). Anchored below the hovered palette button, clamped
    /// to the screen rect. No-op when nothing is hovered or while a
    /// gate drag is in progress.
    ///
    /// Chrome matches the state panel (paper bg + ui-2 1 px border +
    /// soft shadow). Typography follows the Tailwind scale: title
    /// text-sm (14 px) in tx, description text-xs (12 px) in tx-2.
    pub(crate) fn draw_palette_tooltip(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        colors: &Colors,
    ) {
        let Some(index) = self.hovered_palette_index else {
            return;
        };
        if self.dragging.is_some() {
            return;
        }
        let Some(&gate) = PALETTE_GATES.get(index) else {
            return;
        };
        let layout = palette_layout();
        let Some(local) = palette_gate_local_pos(index, &layout) else {
            return;
        };
        let palette_start_x = rect.width() / 2.0 - layout.total_width / 2.0;
        let palette_origin = rect.min + egui::vec2(palette_start_x, PALETTE_ROW_Y);
        let gate_rect = egui::Rect::from_min_size(
            palette_origin + local.to_vec2(),
            egui::vec2(PALETTE_SIZE, PALETTE_SIZE),
        );

        let info = gate.info();

        // ── Text layout — sizes mirror qni's `.tooltip-*` utilities:
        //     title  = `text-lg`  (18 px) `font-bold` tx — qni's
        //              `.tooltip-heading`. We can't render true bold
        //              without bundling a bold font; rely on size +
        //              colour contrast for hierarchy.
        //     para   = `text-sm`  (14 px) tx-2 — `.tooltip-subheading`.
        let title_galley = painter.layout_no_wrap(
            info.name.to_owned(),
            egui::FontId::proportional(18.0),
            colors.text_strong,
        );
        let desc_galleys: Vec<_> = info
            .paragraphs
            .iter()
            .map(|line| {
                painter.layout_no_wrap(
                    (*line).to_owned(),
                    egui::FontId::proportional(14.0),
                    colors.text,
                )
            })
            .collect();

        // ── Diagram geometry. Sizes match qni's `QubitTransitionComponent`:
        //   * QubitCircle = `h-8 w-8`        → 32 × 32 px
        //   * qpu-operation-sm = `1.5rem`    → 24 × 24 px gate body
        //   * arrow_start / arrow_end SVG    → 12 × 24 px each side
        //   * space-x-2 between groups       → 8 px
        // Per row layout:
        //   [amps_from (2 × 32px circle + 8px gap)]
        //   [12px wire][24px gate][12px wire ending in 6px chevron]
        //   [amps_to (same shape)]
        const CIRCLE: f32 = 32.0;
        const CIRCLE_GAP: f32 = 8.0;
        const SECTION_GAP: f32 = 8.0;
        const WIRE: f32 = 12.0;
        const ARROWHEAD: f32 = 6.0;
        const GATE_BODY: f32 = 24.0;
        const ROW_GAP: f32 = 8.0;
        let amps_w = CIRCLE * 2.0 + CIRCLE_GAP;
        // The arrowhead chevron is drawn over the last 6 px of the right
        // wire (matches qni's arrow_end SVG where the chevron tip ends
        // at x=11.6 within a 12 px wire). So the connector width is
        // simply 12 + 24 + 12 — ARROWHEAD is the chevron length used
        // during drawing, not a separate horizontal slot.
        let conn_w = WIRE + GATE_BODY + WIRE;
        let diagram_w = amps_w + SECTION_GAP + conn_w + SECTION_GAP + amps_w;
        let row_h = CIRCLE + 4.0; // room for the basis label tucked into the bottom-right
        let diagram_h = if info.transitions.is_empty() {
            0.0
        } else {
            let n = info.transitions.len() as f32;
            n * row_h + (n - 1.0).max(0.0) * ROW_GAP
        };

        // ── Card sizing — Tailwind values straight from qni's tooltip
        //     theme: `px-4 py-3 rounded-lg`, `.tooltip-subheading-first`
        //     `mt-1`, `.tooltip-subheading-second-and-subsequent`
        //     `mt-0.5`, `.tooltip-body` `mt-4`.
        let pad_x = 16.0_f32; // px-4
        let pad_y = 12.0_f32; // py-3
        let title_gap = 4.0_f32; // mt-1 between title and first paragraph
        let para_gap = 2.0_f32; // mt-0.5 between paragraphs
        let diagram_gap = 16.0_f32; // .tooltip-body { mt-4 }
        let desc_block_h: f32 = if desc_galleys.is_empty() {
            0.0
        } else {
            desc_galleys.iter().map(|g| g.size().y).sum::<f32>()
                + para_gap * (desc_galleys.len() as f32 - 1.0)
        };
        let desc_w = desc_galleys
            .iter()
            .map(|g| g.size().x)
            .fold(0.0_f32, f32::max);
        let content_w = title_galley.size().x.max(desc_w).max(diagram_w);
        let mut content_h = title_galley.size().y;
        if desc_block_h > 0.0 {
            content_h += title_gap + desc_block_h;
        }
        if diagram_h > 0.0 {
            content_h += diagram_gap + diagram_h;
        }
        let card_size = egui::vec2(content_w + pad_x * 2.0, content_h + pad_y * 2.0);

        // ── Anchor below the gate, clamped to the screen rect.
        let anchor = egui::pos2(gate_rect.left(), gate_rect.bottom() + 8.0);
        let max_left = rect.right() - card_size.x - 8.0;
        let max_top = rect.bottom() - card_size.y - 8.0;
        let card_min = egui::pos2(
            anchor.x.min(max_left).max(rect.left() + 8.0),
            anchor.y.min(max_top),
        );
        let card_rect = egui::Rect::from_min_size(card_min, card_size);
        let corner = egui::CornerRadius::same(8); // Tailwind rounded-lg

        let shadow = egui::epaint::Shadow {
            offset: [0, 6],
            blur: 16,
            spread: 0,
            color: egui::Color32::from_rgba_unmultiplied(0, 0, 0, 25),
        };
        painter.add(egui::Shape::Rect(shadow.as_shape(card_rect, corner)));
        painter.rect_filled(card_rect, corner, colors.surface);
        painter.rect_stroke(
            card_rect,
            corner,
            egui::Stroke::new(1.0, colors.box_border),
            egui::StrokeKind::Inside,
        );

        // ── Title.
        let title_pos = card_rect.min + egui::vec2(pad_x, pad_y);
        let title_h = title_galley.size().y;
        painter.galley(title_pos, title_galley, colors.text_strong);

        // ── Description paragraphs.
        let mut cursor_y = title_pos.y + title_h;
        if !desc_galleys.is_empty() {
            cursor_y += title_gap;
        }
        for galley in desc_galleys {
            let h = galley.size().y;
            painter.galley(egui::pos2(title_pos.x, cursor_y), galley, colors.text);
            cursor_y += h + para_gap;
        }

        // ── Diagram (one row per transition).
        if !info.transitions.is_empty() {
            // Trim trailing para_gap before the diagram block.
            let diagram_top = cursor_y
                - if info.paragraphs.is_empty() {
                    0.0
                } else {
                    para_gap
                }
                + diagram_gap;
            let diagram_left = card_rect.center().x - diagram_w / 2.0;
            for (row_idx, trans) in info.transitions.iter().enumerate() {
                let row_top = diagram_top + row_idx as f32 * (row_h + ROW_GAP);
                let row_center_y = row_top + CIRCLE / 2.0;

                // Left amplitudes (input).
                self.draw_tooltip_amps(painter, diagram_left, row_top, &trans.from, colors);
                let mut x = diagram_left + amps_w + SECTION_GAP;

                // Connector: 12 px wire → 24 px gate body → 12 px wire
                // whose last 6 px hold the arrowhead chevron (matches
                // qni's arrow_start / arrow_end SVG geometry). Both
                // wires are pulled 2 px short of the gate edges so the
                // gate sits with a small breathing-room gap on either
                // side instead of being visually fused to the line.
                const WIRE_GATE_PAD: f32 = 2.0;
                let wire_color = colors.text_strong;
                painter.line_segment(
                    [
                        egui::pos2(x, row_center_y),
                        egui::pos2(x + WIRE - WIRE_GATE_PAD, row_center_y),
                    ],
                    egui::Stroke::new(2.0, wire_color),
                );
                let gate_x = x + WIRE;
                let gate_rect_mini = egui::Rect::from_min_size(
                    egui::pos2(gate_x, row_center_y - GATE_BODY / 2.0),
                    egui::vec2(GATE_BODY, GATE_BODY),
                );
                self.draw_tooltip_mini_gate(painter, gate_rect_mini, gate, colors);
                let wire2_x = gate_x + GATE_BODY;
                let arrow_tip = egui::pos2(wire2_x + WIRE, row_center_y);
                // Line ending where the chevron starts (arrow tip −6 px),
                // starting WIRE_GATE_PAD after the gate's right edge.
                painter.line_segment(
                    [
                        egui::pos2(wire2_x + WIRE_GATE_PAD, row_center_y),
                        egui::pos2(arrow_tip.x - ARROWHEAD + 1.0, row_center_y),
                    ],
                    egui::Stroke::new(2.0, wire_color),
                );
                let arrow_base_x = arrow_tip.x - ARROWHEAD;
                painter.add(egui::Shape::convex_polygon(
                    vec![
                        arrow_tip,
                        egui::pos2(arrow_base_x, row_center_y - 4.0),
                        egui::pos2(arrow_base_x, row_center_y + 4.0),
                    ],
                    wire_color,
                    egui::Stroke::NONE,
                ));
                x = arrow_tip.x + SECTION_GAP;

                // Right amplitudes (output).
                self.draw_tooltip_amps(painter, x, row_top, &trans.to, colors);
            }
        }
    }

    /// Render a 2-amplitude row (one `[Amp; 2]`) at the given top-left
    /// position. Each amp = outline circle + filled disk sized by
    /// |amp|² + phase-needle line, with the basis label `|0⟩` / `|1⟩`
    /// tucked into the bottom-right corner (qni convention).
    fn draw_tooltip_amps(
        &self,
        painter: &egui::Painter,
        left_x: f32,
        top_y: f32,
        amps: &[crate::gates::Amp; 2],
        colors: &Colors,
    ) {
        // 32 × 32 px — same size as qni's `qubit-circle` (h-8 w-8).
        const CIRCLE: f32 = 32.0;
        const CIRCLE_GAP: f32 = 8.0;
        for (basis, amp) in amps.iter().enumerate() {
            let center = egui::pos2(
                left_x + CIRCLE / 2.0 + basis as f32 * (CIRCLE + CIRCLE_GAP),
                top_y + CIRCLE / 2.0,
            );
            let prob = amp.probability().clamp(0.0, 1.0);
            let is_zero = prob < 1e-6;
            let outline = if is_zero {
                colors.state_outline_zero
            } else {
                colors.state_outline
            };
            painter.circle_stroke(center, CIRCLE / 2.0, egui::Stroke::new(1.5, outline));
            if !is_zero {
                let inner_r = (CIRCLE / 2.0) * prob.sqrt();
                painter.circle_filled(center, inner_r, colors.state_fill);
                let phase = amp.phase();
                let tip = egui::pos2(
                    center.x + phase.sin() * (CIRCLE / 2.0),
                    center.y - phase.cos() * (CIRCLE / 2.0),
                );
                painter.line_segment([center, tip], egui::Stroke::new(2.0, colors.state_needle));
            }
            // Basis label `|0⟩` / `|1⟩` tucked tight against the
            // circle's bottom-right edge (qni convention). The anchor
            // is the label's top-left, placed at ~5 o'clock just inside
            // the circle's outline so the label's bounding box hugs the
            // disk without floating off to the side.
            let label = if basis == 0 { "|0⟩" } else { "|1⟩" };
            painter.text(
                egui::pos2(center.x + CIRCLE / 2.0 - 7.0, center.y + CIRCLE / 2.0 - 6.0),
                egui::Align2::LEFT_TOP,
                label,
                egui::FontId::monospace(10.0),
                colors.text,
            );
        }
    }

    /// Mini gate body (24 px) for the tooltip diagram — matches qni's
    /// `qpu-operation-sm` (1.5rem). Delegates to the shared
    /// `draw_gate_body` so the icon glyph is identical to what the
    /// palette renders (Phase = `Ø` circle, X = filled disk, …), just
    /// scaled down. Without this every gate would fall back to its
    /// short text `label()` (`P` / `X` / `Ry` / …) and the diagram
    /// wouldn't visually match the palette icon the user is hovering.
    fn draw_tooltip_mini_gate(
        &self,
        painter: &egui::Painter,
        gate_rect: egui::Rect,
        kind: GateKind,
        colors: &Colors,
    ) {
        draw_gate_body(painter, gate_rect, kind, colors);
    }

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
        let gate_rect = egui::Rect::from_min_size(
            circuit_origin + gate.pos.to_vec2(),
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
