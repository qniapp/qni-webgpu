//! Circuit area drawing — qubit lines, placed gates, palette, drag
//! preview. Independent of the state-vector panel.

use eframe::egui;

use crate::app::{CircuitColumnIndex, DragState, GateId, PlacedGate, QniApp};
use crate::colors::{with_alpha, Colors};
use crate::constants::{GATE_SIZE, LINE_GAP, LINE_Y, REM, STEP_OUTSET_Y};
use crate::layout::{gate_size, nearest_slot_index, CircuitProjection, LayoutMetrics};

pub(in crate::render) fn gate_draw_pos(gate: &PlacedGate, drag: Option<DragState>) -> egui::Pos2 {
    drag.filter(|drag| drag.id == gate.id)
        .map_or_else(|| gate.committed_pos(), |drag| drag.preview_pos)
}

/// Screen rect of a gate drawn at `pos` (its circuit-space top-left).
pub(in crate::render) fn gate_screen_rect(
    projection: CircuitProjection,
    gate: &PlacedGate,
    pos: egui::Pos2,
) -> egui::Rect {
    projection.to_screen_rect_from_min_size(pos, gate_size(gate.kind, gate.span.get()))
}

/// Screen centre of the `GATE_SIZE` square whose circuit-space top-left is
/// `pos`.
pub(in crate::render) fn gate_square_center(
    projection: CircuitProjection,
    pos: egui::Pos2,
) -> egui::Pos2 {
    projection.to_screen(pos)
        + projection.to_screen_vec(egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0))
}

const SLOT_CENTER_EPSILON: f32 = 0.5;

pub(super) fn gate_slot_index_for_render(
    gate: &PlacedGate,
    metrics: &LayoutMetrics,
    drag: Option<DragState>,
) -> Option<usize> {
    if drag.is_some_and(|drag| drag.id == gate.id) {
        // A dragged gate can snap to an insert preview halfway between real
        // slots. Do not coerce that preview back to the nearest slot for
        // connector drawing; otherwise a vertical connector is pulled away
        // from the gate center until drop shifts the grid.
        let center_x = gate_draw_pos(gate, drag).x + GATE_SIZE / 2.0;
        let (slot_index, distance) = nearest_slot_index(center_x, &metrics.slot_centers)?;
        return (distance <= SLOT_CENTER_EPSILON).then_some(slot_index);
    }
    (gate.column.as_usize() < metrics.slot_centers.len()).then_some(gate.column.as_usize())
}

impl QniApp {
    pub(crate) fn circuit_content_height(
        &self,
        qubit_count: usize,
        screen_rect: egui::Rect,
    ) -> f32 {
        let line_count = qubit_count.max(1);
        let origin_y = self
            .circuit_projection(screen_rect, 0.0)
            .to_screen(egui::Pos2::ZERO)
            .y
            - screen_rect.min.y;
        let last_line_y = origin_y + LINE_Y + LINE_GAP * (line_count.saturating_sub(1)) as f32;
        let content_height = if matches!(self.mode, crate::app::AppMode::Embed { .. }) {
            // 16px: embeds do not need the standalone editor's drop-area padding.
            last_line_y + GATE_SIZE + crate::tailwind::SPACING_4 + self.circuit_block_outset_y()
        } else {
            last_line_y + GATE_SIZE + 4.0 * REM
        };
        content_height.max(screen_rect.height())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_circuit(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        metrics: &LayoutMetrics,
        colors: &Colors,
        fast_drag: bool,
        dragging_gate_id: Option<GateId>,
        scroll_time: f32,
    ) -> (bool, super::circuit_gates::CircuitGpuBatch) {
        // The projection maps circuit space onto `rect`: shifted by the
        // narrow-canvas gutter, the time scroll, the restricted-palette shift
        // and the circuit blocks' outset, and rotated in the vertical layout.
        // Anything pinned to the circuit's coordinate system (wires, slot
        // grid, gate bodies, step indicators, connectors) is drawn through
        // it; the GPU callback viewports stay on `rect` so they don't track
        // the scroll.
        let projection = self.circuit_projection(rect, scroll_time);
        // Blocks sit behind everything else, like qni's block body that
        // wraps its steps.
        self.draw_circuit_blocks(painter, metrics, colors, projection);
        for &line_y in &metrics.line_ys {
            let start = projection.to_screen(egui::pos2(metrics.line_left, line_y));
            let end = projection.to_screen(egui::pos2(metrics.line_right, line_y));
            painter.line_segment([start, end], egui::Stroke::new(2.0_f32, colors.line));
        }

        // Step-preview bars at the trailing edge of the hovered /
        // breakpoint column (leading edge of slot 0 for INITIAL).
        // Hovered = 30% alpha (live preview), breakpoint = full opacity.
        // Mirrors
        // qni's `circuit-step::after` data-active / data-breakpoint
        // styling.
        if !metrics.line_ys.is_empty() && !metrics.slot_centers.is_empty() {
            let top = metrics.line_ys[0] - STEP_OUTSET_Y;
            let bot = metrics.line_ys[metrics.line_ys.len() - 1] + STEP_OUTSET_Y;
            let step_line = |painter: &egui::Painter, slot: usize, alpha: u8| {
                let x = if slot == CircuitColumnIndex::INITIAL.as_usize() {
                    metrics.slot_centers[0] - crate::constants::SLOT_SPACING * 0.5
                } else if let Some(center) = metrics.slot_centers.get(slot) {
                    center + crate::constants::SLOT_SPACING * 0.5
                } else {
                    return;
                };
                let color = with_alpha(colors.step_preview, alpha);
                painter.line_segment(
                    [
                        projection.to_screen(egui::pos2(x, top)),
                        projection.to_screen(egui::pos2(x, bot)),
                    ],
                    egui::Stroke::new(4.0_f32, color),
                );
            };
            if let Some(step) = self.breakpoint_step {
                step_line(painter, step.as_usize(), 255);
            }
            if let Some(step) = self.hovered_step {
                if Some(step) != self.breakpoint_step {
                    step_line(painter, step.as_usize(), 80);
                }
            }
        }

        self.draw_circuit_connectors(painter, metrics, colors, projection, dragging_gate_id);

        // Keep one visible viewport for GPU geometry even when insert layering
        // splits its paint scissor into left and right passes.
        let gpu_viewport = rect.intersect(painter.clip_rect());
        let batch =
            self.build_circuit_gpu_batch(gpu_viewport, projection, dragging_gate_id, colors);
        let insert_preview_painted = self.draw_placed_circuit_gates(
            painter,
            projection,
            colors,
            fast_drag,
            dragging_gate_id,
            &batch,
        );

        let mut gpu_clip = painter.clip_rect();
        if insert_preview_painted {
            if let Some(gate) = self
                .placed_gates
                .iter()
                .find(|gate| Some(gate.id) == dragging_gate_id)
            {
                gpu_clip = super::circuit_gates::insert_preview_right_clip(
                    gpu_clip,
                    projection,
                    gate_draw_pos(gate, self.dragging),
                );
            }
        }
        if gpu_clip.is_positive() {
            batch.paint_overlays(&painter.with_clip_rect(gpu_clip));
        }

        let gutters = self.circuit_gutters(rect);
        for (index, &line_y) in metrics.line_ys.iter().enumerate() {
            // Labels live in circuit space (anchored to the wire's
            // start) so they scroll with the rest of the circuit —
            // otherwise the leftmost gates would slide under fixed
            // "q0:" / "q1:" labels and visually collide.
            let (label_pos, label) = if projection.is_vertical() {
                (
                    projection.to_screen(egui::pos2(gutters.label_x, line_y)),
                    format!("q{index}"),
                )
            } else {
                (
                    projection.to_screen(egui::pos2(gutters.label_x, line_y - 7.0)),
                    format!("q{index}:"),
                )
            };
            painter.text(
                label_pos,
                gutters.label_align,
                label,
                // Monospace keeps q0:/q1: wire
                // labels aligned with angle labels and state-panel numerals.
                egui::FontId::monospace(crate::tailwind::TEXT_SM),
                colors.text,
            );
        }
        (insert_preview_painted, batch)
    }
}

#[cfg(test)]
mod tests {
    use crate::app::PlacedGate;
    use crate::constants::SLOT_SPACING;
    use crate::gates::GateKind;
    use crate::layout::layout_metrics;

    #[test]
    fn dragged_insert_preview_does_not_join_slot_connector() {
        let metrics = layout_metrics(600.0, 2, 3);
        let gate = PlacedGate::new(
            crate::app::GateId::from_u32(1),
            GateKind::Control,
            crate::app::CircuitColumnIndex::new(0),
            crate::app::WireIndex::new(0),
            crate::gates::GateSpan::SINGLE,
            None,
        );
        let preview_pos = eframe::egui::pos2(
            metrics.slot_centers[0] + SLOT_SPACING * 0.5 - crate::constants::GATE_SIZE * 0.5,
            gate.committed_pos().y,
        );

        let slot = super::gate_slot_index_for_render(
            &gate,
            &metrics,
            Some(crate::app::DragState {
                id: gate.id,
                preview_pos,
                offset: eframe::egui::Vec2::ZERO,
                original_column: Some(gate.column),
                click_copy: None,
            }),
        );

        assert_eq!(slot, None);
    }

    #[test]
    fn dragged_slot_preview_joins_that_slot_connector() {
        let metrics = layout_metrics(600.0, 2, 3);
        let gate = PlacedGate::new(
            crate::app::GateId::from_u32(1),
            GateKind::Control,
            crate::app::CircuitColumnIndex::new(0),
            crate::app::WireIndex::new(0),
            crate::gates::GateSpan::SINGLE,
            None,
        );

        let preview_pos = gate.committed_pos();
        let slot = super::gate_slot_index_for_render(
            &gate,
            &metrics,
            Some(crate::app::DragState {
                id: gate.id,
                preview_pos,
                offset: eframe::egui::Vec2::ZERO,
                original_column: Some(gate.column),
                click_copy: None,
            }),
        );

        assert_eq!(slot, Some(0));
    }
}
