use eframe::egui;
use std::cmp::Ordering;
use std::collections::HashMap;

use crate::icons::{draw_gate_body, draw_gate_body_fast};
use crate::layout::{nearest_slot_index, LayoutMetrics};
use crate::{
    should_use_fast_gate_body, Colors, GateKind, PlacedGate, QniApp, CIRCUIT_PADDING, GATE_SIZE,
    LINE_GAP, LINE_Y, PALETTE_GAP, PALETTE_GATES, PALETTE_ROW_Y, PALETTE_SIZE, REM,
    SNAP_DISTANCE,
};

pub(super) struct StatePanelLayout {
    pub(super) state_count: usize,
    pub(super) qubits: usize,
    pub(super) columns: usize,
    pub(super) size: f32,
    pub(super) gap: f32,
    pub(super) radius: f32,
    pub(super) stroke: f32,
    pub(super) inner_radius: f32,
    pub(super) base_pos: egui::Pos2,
    pub(super) state_rect: egui::Rect,
    pub(super) handle_height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct StateInstanceKey {
    pub(super) state_count: usize,
    pub(super) columns: usize,
    pub(super) size: f32,
    pub(super) gap: f32,
    pub(super) radius: f32,
    pub(super) inner_radius: f32,
    pub(super) stroke: f32,
    pub(super) origin: egui::Pos2,
}

pub(super) struct StateInstanceCache {
    pub(super) key: StateInstanceKey,
    pub(super) instances: std::sync::Arc<[crate::gpu::StateInstance]>,
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
                    let entry = control_groups.entry(slot_index).or_insert((Vec::new(), Vec::new()));
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
            if should_use_fast_gate_body(fast_drag, self.dragging, gate.id) {
                draw_gate_body_fast(painter, gate_rect, gate.kind, colors);
            } else {
                draw_gate_body(painter, gate_rect, gate.kind, colors);
            }
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
}
