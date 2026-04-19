use eframe::egui;
use eframe::{egui_wgpu, wgpu};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

use crate::app::{PlacedGate, QniApp};
use crate::colors::Colors;
use crate::constants::{
    CIRCUIT_PADDING, GATE_SIZE, LINE_GAP, LINE_Y, PALETTE_GAP, PALETTE_GATES, PALETTE_ROW_Y,
    PALETTE_SIZE, REM, SNAP_DISTANCE, STATE_CIRCLE_BOTTOM_MARGIN, STATE_CIRCLE_GAP,
    STATE_CIRCLE_SIZE, STATE_CIRCLE_STROKE,
};
use crate::gates::GateKind;
use crate::gpu::{RenderColors, StateInstance, StateVectorCallback};
use crate::icons::draw_gate_body;
use crate::layout::{layout_metrics, nearest_slot_index, LayoutMetrics};
use crate::shared::{amplitude_qubits, display_index_to_state_index};

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

#[derive(Clone, Copy, Debug, PartialEq)]
struct StateInstanceKey {
    state_count: usize,
    columns: usize,
    size: f32,
    gap: f32,
    radius: f32,
    inner_radius: f32,
    stroke: f32,
    origin: egui::Pos2,
}

pub(super) struct StateInstanceCache {
    key: StateInstanceKey,
    instances: Arc<[StateInstance]>,
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
                let is_control = gate.kind == GateKind::Control;
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
            }
            draw_gate_body(painter, gate_rect, gate.kind, colors);
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
        let palette_width = PALETTE_GATES.len() as f32 * PALETTE_SIZE
            + (PALETTE_GATES.len() as f32 - 1.0) * PALETTE_GAP;
        let palette_start_x = rect.width() / 2.0 - palette_width / 2.0;
        let palette_padding = 1.0 * REM;
        let palette_rect = egui::Rect::from_min_size(
            rect.min
                + egui::vec2(
                    palette_start_x - palette_padding,
                    PALETTE_ROW_Y - palette_padding,
                ),
            egui::vec2(
                palette_width + palette_padding * 2.0,
                PALETTE_SIZE + palette_padding * 2.0,
            ),
        );
        let palette_corner = egui::CornerRadius::same(14);
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

        for (index, gate) in PALETTE_GATES.iter().enumerate() {
            let gate_x = palette_start_x + index as f32 * (PALETTE_SIZE + PALETTE_GAP);
            let gate_rect = egui::Rect::from_min_size(
                rect.min + egui::vec2(gate_x, PALETTE_ROW_Y),
                egui::vec2(PALETTE_SIZE, PALETTE_SIZE),
            );
            if self.hovered_palette_index == Some(index) {
                let hover_outer = gate_rect.expand(4.0);
                let hover_inner = gate_rect.expand(2.0);
                painter.rect_filled(hover_outer, egui::CornerRadius::same(10), colors.box_border);
                painter.rect_filled(hover_inner, egui::CornerRadius::same(8), colors.background);
            }
            draw_gate_body(painter, gate_rect, *gate, colors);
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
        draw_gate_body(painter, gate_rect, gate.kind, colors);
    }

    pub(super) fn state_panel_layout(
        &self,
        rect: egui::Rect,
        state_count: usize,
    ) -> StatePanelLayout {
        let state_count = state_count.max(1);
        let qubits = amplitude_qubits(state_count);
        let gap_ratio = STATE_CIRCLE_GAP / STATE_CIRCLE_SIZE;
        let state_padding = (1.0 * REM)
            .min(rect.width() * 0.05)
            .min(rect.height() * 0.05);
        let top_limit = rect.min.y + PALETTE_ROW_Y + PALETTE_SIZE + 2.0 * REM;
        let mut available_width = rect.width() - state_padding * 2.0;
        let mut available_height = rect.max.y - STATE_CIRCLE_BOTTOM_MARGIN - top_limit;
        if available_width <= 0.0 {
            available_width = rect.width().max(1.0);
        }
        if available_height <= 0.0 {
            available_height = (rect.height() - STATE_CIRCLE_BOTTOM_MARGIN).max(1.0);
        }
        let max_fraction = if state_count <= 4 {
            0.4
        } else if state_count <= 16 {
            0.3
        } else {
            0.25
        };
        let max_height = rect.height() * max_fraction;
        if available_height > max_height {
            available_height = max_height.max(1.0);
        }

        let aspect = (available_width / available_height).max(0.1);
        let mut columns = 1usize;
        let mut rows = state_count;
        let mut best_size = 0.0;
        let mut best_score = f32::INFINITY;
        for candidate in 1..=state_count {
            if !state_count.is_multiple_of(candidate) {
                continue;
            }
            let candidate_rows = state_count / candidate;
            let size_w = available_width / (candidate as f32 + (candidate - 1) as f32 * gap_ratio);
            let size_h = available_height
                / (candidate_rows as f32 + (candidate_rows - 1) as f32 * gap_ratio);
            let size = size_w.min(size_h).clamp(0.5, STATE_CIRCLE_SIZE);
            let ratio = candidate as f32 / candidate_rows as f32;
            let score = (ratio - aspect).abs();
            if size > best_size + 0.01 || ((size - best_size).abs() <= 0.01 && score < best_score) {
                columns = candidate;
                rows = candidate_rows;
                best_size = size;
                best_score = score;
            }
        }
        let size = best_size.max(0.5);
        let gap = size * gap_ratio;
        let total_width = size * columns as f32 + gap * (columns.saturating_sub(1)) as f32;
        let total_height = size * rows as f32 + gap * (rows.saturating_sub(1)) as f32;
        let base_x = rect.width() / 2.0 - total_width / 2.0;
        let base_y = rect.height() - STATE_CIRCLE_BOTTOM_MARGIN - total_height;
        let radius = size * 0.5;
        let stroke = STATE_CIRCLE_STROKE.min(size * 0.25).max(0.5);
        let scale = size / STATE_CIRCLE_SIZE;
        let inner_radius = (radius - stroke * 0.5 + 0.5 * scale).max(0.0);

        let content_height = total_height + state_padding * 2.0;
        // Keep the grip bar tall enough to stay easy to grab even when the circles shrink.
        let handle_height = (0.4 * REM).min(content_height * 0.4).max(10.0);
        // Reserve half a handle of padding so the drag affordance reads as separate from the circles.
        let handle_padding = handle_height * 0.5;

        let base_pos = rect.min + egui::vec2(base_x, base_y);
        let state_rect = egui::Rect::from_min_size(
            // Balance the extra top handle space with the regular circle padding so the panel still feels centered.
            base_pos
                - egui::vec2(
                    state_padding,
                    state_padding + handle_height + handle_padding,
                ),
            egui::vec2(
                total_width + state_padding * 2.0,
                total_height + state_padding * 2.0 + handle_height + handle_padding,
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
        let min_x = rect.min.x;
        let max_x = rect.max.x - layout.state_rect.width();
        let min_y = rect.min.y;
        let max_y = rect.max.y - layout.state_rect.height();
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

    fn state_instances_for(
        &mut self,
        layout: &StatePanelLayout,
        origin: egui::Pos2,
    ) -> (Arc<[StateInstance]>, bool) {
        let key = StateInstanceKey {
            state_count: layout.state_count,
            columns: layout.columns,
            size: layout.size,
            gap: layout.gap,
            radius: layout.radius,
            inner_radius: layout.inner_radius,
            stroke: layout.stroke,
            origin,
        };
        if let Some(cache) = &self.state_instance_cache {
            if cache.key == key {
                return (cache.instances.clone(), false);
            }
        }

        let mut instances = Vec::with_capacity(layout.state_count);
        for i in 0..layout.state_count {
            let state_index = display_index_to_state_index(i, layout.qubits) as u32;
            let row = i / layout.columns;
            let col = i % layout.columns;
            let x = origin.x + col as f32 * (layout.size + layout.gap);
            let y = origin.y + row as f32 * (layout.size + layout.gap);
            instances.push(StateInstance {
                center: [x + layout.radius, y + layout.radius],
                radius: layout.radius,
                inner_radius: layout.inner_radius,
                stroke: layout.stroke,
                state_index,
            });
        }
        let instances: Arc<[StateInstance]> = instances.into();
        self.state_instance_cache = Some(StateInstanceCache {
            key,
            instances: instances.clone(),
        });
        (instances, true)
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

        let handle_rect = egui::Rect::from_min_size(
            state_rect.min,
            egui::vec2(state_rect.width(), handle_height.max(6.0)),
        );
        painter.rect_filled(handle_rect, state_corner, colors.box_border);
        let grip_width = handle_rect.width() * 0.25;
        let grip_height = handle_height * 0.25;
        let grip_rect = egui::Rect::from_center_size(
            handle_rect.center(),
            egui::vec2(grip_width, grip_height.max(2.0)),
        );
        painter.rect_filled(grip_rect, egui::CornerRadius::same(4), colors.surface);

        if let Some(target_format) = target_format {
            let (instances, instances_dirty) = self.state_instances_for(layout, base_pos);
            let gate_params = if recompute {
                let metrics = layout_metrics(screen_rect.width(), layout.qubits);
                self.collect_gate_params(layout.qubits, layout.state_count, &metrics)
            } else {
                Vec::new()
            };
            let render_colors = RenderColors::new(colors);
            let callback = StateVectorCallback {
                instances,
                instances_dirty,
                gate_params,
                state_count: layout.state_count,
                recompute,
                target_format,
                colors: render_colors,
            };
            let callback_rect = screen_rect;
            let clipped = painter.with_clip_rect(state_rect);
            let paint_callback = egui_wgpu::Callback::new_paint_callback(callback_rect, callback);
            clipped.add(egui::Shape::Callback(paint_callback));
        }

        handle_rect
    }
}
