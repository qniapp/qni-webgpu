use eframe::egui;
use eframe::{egui_wgpu, wgpu};
use std::cmp::Ordering;
use std::collections::HashMap;

use crate::app::{PlacedGate, QniApp, ResizeCorner};
use crate::colors::Colors;
use crate::constants::{
    state_circle_layout, CIRCUIT_PADDING, GATE_SIZE, LINE_GAP, LINE_Y, PALETTE_CORNER_RADIUS,
    PALETTE_GATES, PALETTE_PADDING_X, PALETTE_PADDING_Y, PALETTE_ROW_Y, PALETTE_SIZE, REM,
    SNAP_DISTANCE, STATE_CIRCLE_BOTTOM_MARGIN, STATE_HANDLE_HEIGHT, STATE_PANEL_CORNER_RADIUS,
    STATE_RESIZE_HANDLE_PAD, STATE_RESIZE_HANDLE_STROKE, STATE_RESIZE_HIT_PAD,
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
    /// Total pixel size of the circle grid (cols × cell_pitch, rows × cell_pitch).
    grid_size: egui::Vec2,
    /// Inner area below the header strip where circles render. Fixed size;
    /// when the grid is smaller it gets centred, when larger it pans inside.
    pub(super) viewport_rect: egui::Rect,
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

        // Cell size + line width follow qni's per-qubit-count table; the
        // (cols, rows) split is parameterised by `self.aspect_index` so
        // the user can change the layout aspect at runtime. qni's
        // reference uses gap == stroke (cells touch); we add 1 px so
        // adjacent stroke rings don't share a pixel boundary at dist ==
        // outer. Without this slack the GPU-side single-cell render
        // gives 50 % alpha at the boundary (symmetric smoothstep midpoint
        // is exactly 0.5), visibly fading the outline. The 1-px seam is
        // barely perceptible at typical zoom and lets us keep V-sync at
        // 11+ qubits without paying for 2x2 cell sampling in the
        // fragment shader.
        let qni = state_circle_layout(qubits, self.aspect_index);
        let columns = qni.cols;
        let rows = qni.rows;
        // Zoom scales every length-y thing in the grid uniformly so cells
        // grow / shrink together. Stroke has a 0.5 px floor so very-zoomed-
        // out cells still get a visible outline rather than collapsing into
        // pure fill.
        let zoom = self.state_grid_zoom;
        let size = qni.size * zoom;
        let stroke = (qni.line_width * zoom).max(0.5);
        let gap = (qni.line_width + 1.0) * zoom;

        let total_width = size * columns as f32 + gap * (columns.saturating_sub(1)) as f32;
        let total_height = size * rows as f32 + gap * (rows.saturating_sub(1)) as f32;
        let radius = size * 0.5;
        let inner_radius = (radius - stroke * 0.5).max(0.0);

        // qni-style header strip (G-2): fixed-height zinc-100 bar showing
        // qubit count + grid dims. Drag-to-move is the only interaction
        // attached to the strip for now (resize handles are TBD).
        let handle_height = STATE_HANDLE_HEIGHT;

        // Make sure the panel is wide enough that the strip's left/right
        // labels never overlap. Hack monospace at 11 px is ≈7 px / glyph;
        // budget a bit extra for the multiplication sign.
        const STRIP_CHAR_WIDTH: f32 = 7.0;
        const STRIP_PADDING_X: f32 = 12.0;
        const STRIP_LABEL_GAP: f32 = 16.0;
        let qubits_label = if qubits == 1 { "qubit" } else { "qubits" };
        let states_label = if state_count == 1 { "state" } else { "states" };
        let qubits_chars = format!("{qubits} {qubits_label}").chars().count();
        // "+ 2" reserves room for the " ▾" suffix on the right text that
        // signals the aspect popover is openable.
        let states_chars = format!("{columns} × {rows} = {state_count} {states_label}")
            .chars()
            .count()
            + 2;
        let strip_min_width = (qubits_chars + states_chars) as f32 * STRIP_CHAR_WIDTH
            + STRIP_PADDING_X * 2.0
            + STRIP_LABEL_GAP;

        // Panel size is user-controlled (resize via the corner L-handles).
        // Strip-text minimum is the only thing that can force `panel_width`
        // above the user's choice — practically a no-op for ≤16 qubits
        // since min viewport width already covers the widest label.
        let panel_width = self.state_viewport_size.x.max(strip_min_width);
        let panel_height = self.state_viewport_size.y + handle_height;
        let panel_min_x = rect.width() / 2.0 - panel_width / 2.0;
        let panel_min_y = rect.height() - STATE_CIRCLE_BOTTOM_MARGIN - panel_height;
        let state_rect = egui::Rect::from_min_size(
            rect.min + egui::vec2(panel_min_x, panel_min_y),
            egui::vec2(panel_width, panel_height),
        );
        let viewport_rect = egui::Rect::from_min_max(
            state_rect.min + egui::vec2(0.0, handle_height),
            state_rect.max,
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
            grid_size: egui::vec2(total_width, total_height),
            viewport_rect,
            state_rect,
            handle_height,
        }
    }

    /// Where the circle grid's top-left corner should render given the panel
    /// layout and the user's pan offset. If the grid fits inside the
    /// viewport on an axis it gets centred (and the pan offset is ignored
    /// for that axis); otherwise the offset is clamped so the grid can pan
    /// only as far as its edges meet the viewport edges.
    pub(super) fn grid_origin(
        layout: &StatePanelLayout,
        viewport_offset: egui::Vec2,
        pan: egui::Vec2,
    ) -> egui::Pos2 {
        let viewport = layout.viewport_rect.translate(viewport_offset);
        let grid = layout.grid_size;
        let origin_x = if grid.x <= viewport.width() {
            viewport.min.x + (viewport.width() - grid.x) / 2.0
        } else {
            // Grid wider than viewport — clamp pan so the grid edges can't
            // separate from the viewport edges (no empty bands either side).
            let min = viewport.max.x - grid.x;
            let max = viewport.min.x;
            (viewport.min.x + pan.x).clamp(min, max)
        };
        let origin_y = if grid.y <= viewport.height() {
            viewport.min.y + (viewport.height() - grid.y) / 2.0
        } else {
            let min = viewport.max.y - grid.y;
            let max = viewport.min.y;
            (viewport.min.y + pan.y).clamp(min, max)
        };
        egui::pos2(origin_x, origin_y)
    }

    /// Hit rect for the strip's dimensions text ("C × R = N states ▾"),
    /// which is wheel-scrollable for aspect ±1 and click-able to open the
    /// aspect popover. Computed by measuring the text with the actual font
    /// so the rect exactly tracks the rendered glyphs; expanded by a few
    /// px on all sides for forgiving clicks.
    pub(crate) fn dims_text(layout: &StatePanelLayout) -> String {
        let states_label = if layout.state_count == 1 {
            "state"
        } else {
            "states"
        };
        let rows = layout.state_count / layout.columns.max(1);
        format!(
            "{} × {} = {} {} ▾",
            layout.columns, rows, layout.state_count, states_label
        )
    }

    pub(super) fn dims_hit_rect(
        ctx: &egui::Context,
        layout: &StatePanelLayout,
        state_panel_offset: egui::Vec2,
    ) -> egui::Rect {
        let state_rect = layout.state_rect.translate(state_panel_offset);
        let handle_rect = egui::Rect::from_min_size(
            state_rect.min,
            egui::vec2(state_rect.width(), layout.handle_height.max(6.0)),
        );
        let strip_padding_x = STATE_PANEL_CORNER_RADIUS + 6.0;
        let font = egui::FontId::monospace(11.0);
        let text = Self::dims_text(layout);
        let size = ctx.fonts_mut(|f| {
            f.layout_no_wrap(text, font, egui::Color32::WHITE).size()
        });
        let right_center = handle_rect.right_center() - egui::vec2(strip_padding_x, 0.0);
        let visible = egui::Rect::from_min_max(
            egui::pos2(right_center.x - size.x, right_center.y - size.y / 2.0),
            egui::pos2(right_center.x, right_center.y + size.y / 2.0),
        );
        visible.expand2(egui::vec2(6.0, 4.0))
    }

    /// Aspect popover (D 案) layout. Anchored to the bottom-right corner
    /// of the dimensions text, opening downward. Each row corresponds to
    /// one `aspect_index ∈ [0, qubits]` choice. Returns the popover rect
    /// (for outside-click detection) plus a Vec of per-row rects (for
    /// click-to-pick interaction and matching draw geometry).
    pub(super) fn aspect_popover_layout(
        dims_rect: egui::Rect,
        qubits: usize,
    ) -> (egui::Rect, Vec<egui::Rect>) {
        const ROW_HEIGHT: f32 = 22.0;
        const PADDING: f32 = 8.0;
        const WIDTH: f32 = 240.0;
        const MAX_HEIGHT: f32 = 420.0;
        let n_rows = qubits + 1;
        let content_height = n_rows as f32 * ROW_HEIGHT;
        let total_height = (content_height + PADDING * 2.0).min(MAX_HEIGHT);
        let rect = egui::Rect::from_min_size(
            egui::pos2(dims_rect.max.x - WIDTH, dims_rect.max.y + 2.0),
            egui::vec2(WIDTH, total_height),
        );
        let mut rows = Vec::with_capacity(n_rows);
        for i in 0..n_rows {
            let y = rect.min.y + PADDING + (i as f32 * ROW_HEIGHT);
            rows.push(egui::Rect::from_min_size(
                egui::pos2(rect.min.x + PADDING, y),
                egui::vec2(WIDTH - PADDING * 2.0, ROW_HEIGHT - 2.0),
            ));
        }
        (rect, rows)
    }

    /// Draw the aspect popover (background + rows). Each row shows an
    /// aspect-correct thumbnail rect, the cols × rows label, and a "✓"
    /// for the current selection. Currently a fixed-height popover with
    /// up to qubits+1 rows; for 16 qubits that's 17 rows × 22 px ≈ 374 px,
    /// which fits inside `MAX_HEIGHT = 420`.
    pub(super) fn draw_aspect_popover(
        painter: &egui::Painter,
        colors: &Colors,
        rect: egui::Rect,
        rows: &[egui::Rect],
        qubits: usize,
        current_aspect: usize,
    ) {
        // Drop shadow behind the popover so it lifts above the panel.
        let corner = egui::CornerRadius::same(10);
        let shadow = egui::epaint::Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: egui::Color32::from_rgba_unmultiplied(0, 0, 0, 46),
        };
        painter.add(egui::Shape::Rect(shadow.as_shape(rect, corner)));
        painter.rect_filled(rect, corner, colors.surface);

        let label_font = egui::FontId::monospace(12.0);
        const THUMB_SLOT_W: f32 = 50.0;
        const THUMB_SLOT_H: f32 = 16.0;
        for (i, row_rect) in rows.iter().enumerate() {
            let is_current = i == current_aspect;
            let cols = 1usize << i;
            let layout_rows = 1usize << (qubits - i);
            // Row background (current = sky-500, else hover-ready surface).
            if is_current {
                painter.rect_filled(*row_rect, egui::CornerRadius::same(6), colors.state_fill);
            }
            // Thumbnail (aspect-correct rect) inside a fixed 50×16 slot.
            let slot_min = egui::pos2(
                row_rect.min.x + 8.0,
                row_rect.center().y - THUMB_SLOT_H / 2.0,
            );
            let slot_rect =
                egui::Rect::from_min_size(slot_min, egui::vec2(THUMB_SLOT_W, THUMB_SLOT_H));
            let aspect_scale =
                (THUMB_SLOT_W / cols as f32).min(THUMB_SLOT_H / layout_rows as f32);
            let thumb_w = (cols as f32 * aspect_scale).max(1.0);
            let thumb_h = (layout_rows as f32 * aspect_scale).max(1.0);
            let thumb_min = egui::pos2(
                slot_rect.center().x - thumb_w / 2.0,
                slot_rect.center().y - thumb_h / 2.0,
            );
            let thumb_color = if is_current {
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 220)
            } else {
                egui::Color32::from_rgba_unmultiplied(82, 82, 91, 180) // zinc-600 60%
            };
            painter.rect_filled(
                egui::Rect::from_min_size(thumb_min, egui::vec2(thumb_w, thumb_h)),
                egui::CornerRadius::ZERO,
                thumb_color,
            );
            // Label
            let label = format!("{} × {}", cols, layout_rows);
            let label_color = if is_current {
                egui::Color32::WHITE
            } else {
                colors.text
            };
            painter.text(
                egui::pos2(row_rect.min.x + 8.0 + THUMB_SLOT_W + 12.0, row_rect.center().y),
                egui::Align2::LEFT_CENTER,
                label,
                label_font.clone(),
                label_color,
            );
            // "(now)" tag for the current row — visible without depending
            // on the ✓ glyph (Hack font ships a generic box for it).
            if is_current {
                painter.text(
                    egui::pos2(row_rect.max.x - 10.0, row_rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    "(now)",
                    egui::FontId::monospace(10.0),
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 220),
                );
            }
        }
    }

    /// Hit rect for grabbing one corner. The visible handle is an arc of
    /// the panel's rounded inner edge, but for clicks we expose the full
    /// `R × R` square at the corner (the panel's rounded-corner bounding
    /// box) inflated by `STATE_RESIZE_HIT_PAD` so the corner is forgiving
    /// to grab even when the cursor isn't right on the curve.
    pub(super) fn resize_handle_hit_rect(
        layout: &StatePanelLayout,
        offset: egui::Vec2,
        corner: ResizeCorner,
    ) -> egui::Rect {
        let state_rect = layout.state_rect.translate(offset);
        let r = STATE_PANEL_CORNER_RADIUS;
        let base = match corner {
            ResizeCorner::TopLeft => {
                egui::Rect::from_min_size(state_rect.min, egui::vec2(r, r))
            }
            ResizeCorner::TopRight => egui::Rect::from_min_size(
                egui::pos2(state_rect.max.x - r, state_rect.min.y),
                egui::vec2(r, r),
            ),
            ResizeCorner::BottomLeft => egui::Rect::from_min_size(
                egui::pos2(state_rect.min.x, state_rect.max.y - r),
                egui::vec2(r, r),
            ),
            ResizeCorner::BottomRight => egui::Rect::from_min_size(
                egui::pos2(state_rect.max.x - r, state_rect.max.y - r),
                egui::vec2(r, r),
            ),
        };
        base.expand(STATE_RESIZE_HIT_PAD)
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

    /// Keep `state_grid_offset` inside the range that lets `grid_origin`
    /// produce a non-flickering value: 0 when the grid fits on an axis,
    /// `[viewport - grid, 0]` when it overflows. Called every frame after
    /// the layout is computed so qubit-count changes don't leave a stale
    /// (possibly huge) pan offset around.
    pub(super) fn clamp_state_grid_offset(&mut self, layout: &StatePanelLayout) {
        let viewport = layout.viewport_rect.translate(self.state_panel_offset);
        let grid = layout.grid_size;
        self.state_grid_offset.x = if grid.x <= viewport.width() {
            0.0
        } else {
            self.state_grid_offset
                .x
                .clamp(viewport.width() - grid.x, 0.0)
        };
        self.state_grid_offset.y = if grid.y <= viewport.height() {
            0.0
        } else {
            self.state_grid_offset
                .y
                .clamp(viewport.height() - grid.y, 0.0)
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
        let viewport_rect = layout.viewport_rect.translate(offset);
        // Where the circle grid actually lands in viewport coords. Centred
        // when the grid fits, panned by `state_grid_offset` otherwise.
        let grid_origin = Self::grid_origin(layout, offset, self.state_grid_offset);
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

        // Strip text starts past the corner resize-handle area
        // (panel rounded R + breathing). Keeps "16 qubits" / "256 × 256 = …"
        // from touching the curved handle marks at the top corners.
        let strip_padding_x = STATE_PANEL_CORNER_RADIUS + 6.0;
        let strip_font = egui::FontId::monospace(11.0);
        let qubits_label = if layout.qubits == 1 { "qubit" } else { "qubits" };
        let states_label = if layout.state_count == 1 { "state" } else { "states" };
        let qubits_text = format!("{} {}", layout.qubits, qubits_label);
        let rows = layout.state_count / layout.columns.max(1);
        // " ▾" indicates the dimensions text opens the aspect popover.
        let states_text = format!(
            "{} × {} = {} {} ▾",
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
                panel_origin: [grid_origin.x, grid_origin.y],
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
            // Clip the GPU pass to the viewport so the grid is cropped at
            // the panel body's inner edge — circles flush against the rounded
            // corners get sliced cleanly instead of bleeding past the panel.
            let clipped = painter.with_clip_rect(viewport_rect);
            let paint_callback = egui_wgpu::Callback::new_paint_callback(callback_rect, callback);
            clipped.add(egui::Shape::Callback(paint_callback));
        }

        Self::draw_state_minimap(painter, layout, viewport_rect, grid_origin);

        // Aspect popover (D 案) — only draw when open. Positioned below
        // the dimensions text; floats above the panel and any minimap.
        if self.aspect_popover_open {
            let dims_rect = Self::dims_hit_rect(
                painter.ctx(),
                layout,
                offset,
            );
            let (pop_rect, row_rects) =
                Self::aspect_popover_layout(dims_rect, layout.qubits);
            Self::draw_aspect_popover(
                painter,
                colors,
                pop_rect,
                &row_rects,
                layout.qubits,
                self.aspect_index.min(layout.qubits),
            );
        }

        // Resize handles — 4 corner arcs concentric with the panel's
        // rounded corners (G 案 / 内側配置). Drawn after the GPU pass so
        // they sit on top of the circle grid at all zoom levels. Color
        // follows the local background: sky-tone for the top handles (on
        // the sky-500 strip), neutral gray for the bottom handles (on the
        // white panel).
        for corner in [
            ResizeCorner::TopLeft,
            ResizeCorner::TopRight,
            ResizeCorner::BottomLeft,
            ResizeCorner::BottomRight,
        ] {
            let dragging = self.active_resize_corner() == Some(corner);
            let color = match (corner.is_top(), dragging) {
                (true, false) => colors.state_resize_handle_top_idle,
                (true, true) => colors.state_resize_handle_top_drag,
                (false, false) => colors.state_resize_handle_bottom_idle,
                (false, true) => colors.state_resize_handle_bottom_drag,
            };
            Self::draw_resize_handle_arc(painter, corner, state_rect, color);
        }

        handle_rect
    }

    /// Draw a single resize-handle arc. The arc is concentric with the
    /// panel's rounded corner: same centre as the panel's corner-radius
    /// circle, radius = `STATE_PANEL_CORNER_RADIUS − STATE_RESIZE_HANDLE_PAD`.
    /// This means the handle literally traces the panel's rounded inner
    /// edge offset inward by `PAD`, so the handle's curvature matches the
    /// panel's R exactly.
    fn draw_resize_handle_arc(
        painter: &egui::Painter,
        corner: ResizeCorner,
        state_rect: egui::Rect,
        color: egui::Color32,
    ) {
        use std::f32::consts::PI;
        let r = STATE_PANEL_CORNER_RADIUS;
        let inner_r = (r - STATE_RESIZE_HANDLE_PAD).max(0.0);
        if inner_r <= 0.0 {
            return;
        }
        // `center` is the centre of the panel's rounded-corner circle for
        // this corner (located `r` px inside the corner along both axes).
        // `start_angle` is the angle on the circle at which the arc begins;
        // we always sweep +π/2 (90°) counterclockwise (in math y-up terms;
        // visually that's "along the corner curve").
        let (center, start_angle) = match corner {
            ResizeCorner::TopLeft => (state_rect.min + egui::vec2(r, r), PI),
            ResizeCorner::TopRight => {
                (egui::pos2(state_rect.max.x - r, state_rect.min.y + r), -PI / 2.0)
            }
            ResizeCorner::BottomRight => (state_rect.max - egui::vec2(r, r), 0.0),
            ResizeCorner::BottomLeft => {
                (egui::pos2(state_rect.min.x + r, state_rect.max.y - r), PI / 2.0)
            }
        };
        const ARC_SEGMENTS: usize = 16;
        let mut points: Vec<egui::Pos2> = Vec::with_capacity(ARC_SEGMENTS + 1);
        for i in 0..=ARC_SEGMENTS {
            let t = i as f32 / ARC_SEGMENTS as f32;
            let a = start_angle + t * (PI / 2.0);
            points.push(center + egui::vec2(inner_r * a.cos(), inner_r * a.sin()));
        }
        painter.add(egui::Shape::line(
            points,
            egui::Stroke::new(STATE_RESIZE_HANDLE_STROKE, color),
        ));
    }

    /// Bottom-right minimap that shows where the viewport is sitting on the
    /// (potentially much larger) grid. The outer rectangle matches the grid
    /// aspect (so a 32×32 grid produces a square minimap, a 32×8 grid a
    /// wide one), and the lighter inset rectangle inside marks the visible
    /// region. Only painted when the grid actually exceeds the viewport on
    /// at least one axis — otherwise the whole grid is on screen and the
    /// minimap is just chrome.
    fn draw_state_minimap(
        painter: &egui::Painter,
        layout: &StatePanelLayout,
        viewport_rect: egui::Rect,
        grid_origin: egui::Pos2,
    ) {
        let grid = layout.grid_size;
        if grid.x <= viewport_rect.width() && grid.y <= viewport_rect.height() {
            return;
        }
        // Grid aspect drives the minimap's outer dimensions, capped at a
        // bounding box. No letterboxing inside — the inset rect IS the grid.
        const MAX_W: f32 = 80.0;
        const MAX_H: f32 = 50.0;
        const INSET: f32 = 3.0;
        let aspect = grid.x / grid.y;
        let (inner_w, inner_h) = if aspect >= MAX_W / MAX_H {
            (MAX_W, MAX_W / aspect)
        } else {
            (MAX_H * aspect, MAX_H)
        };
        let mm_size = egui::vec2(inner_w + INSET * 2.0, inner_h + INSET * 2.0);
        let pad = 6.0;
        let mm_rect = egui::Rect::from_min_max(
            viewport_rect.max - mm_size - egui::vec2(pad, pad),
            viewport_rect.max - egui::vec2(pad, pad),
        );
        let bg = egui::Color32::from_rgba_unmultiplied(0, 0, 0, 140);
        painter.rect_filled(mm_rect, egui::CornerRadius::same(4), bg);

        let mm_grid_min = mm_rect.min + egui::vec2(INSET, INSET);
        let mm_grid_size = egui::vec2(inner_w, inner_h);
        let scale = inner_w / grid.x;
        // Visible region inside the grid, in grid-space pixels.
        let visible_offset = viewport_rect.min - grid_origin;
        let mm_vp_min = mm_grid_min + visible_offset * scale;
        let mm_vp_size = egui::vec2(viewport_rect.width(), viewport_rect.height()) * scale;
        let mm_vp_rect = egui::Rect::from_min_size(mm_vp_min, mm_vp_size)
            .intersect(egui::Rect::from_min_size(mm_grid_min, mm_grid_size));
        let vp_fill = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 70);
        let vp_stroke = egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 220),
        );
        painter.rect_filled(mm_vp_rect, egui::CornerRadius::ZERO, vp_fill);
        painter.rect_stroke(
            mm_vp_rect,
            egui::CornerRadius::ZERO,
            vp_stroke,
            egui::StrokeKind::Inside,
        );
    }
}
