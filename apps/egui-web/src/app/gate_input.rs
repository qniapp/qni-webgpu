//! Gate input — pickup, drag-to-place, drop, hover detection — plus
//! the drag-time repaint throttle. Independent of the state panel.

use eframe::egui;
use std::time::Duration;

use super::{DragState, PlacedGate, QftResizeDrag, QniApp};
use crate::constants::{
    DRAG_REPAINT_BASE_SECS, DRAG_REPAINT_MAX_SECS, DRAG_REPAINT_MIN_SECS,
    DRAG_REPAINT_PUMP_FACTOR, GATE_SIZE, LINE_GAP, PALETTE_GATES, PALETTE_ROW_Y, QFT_MAX_SPAN,
    SLOT_SPACING, SNAP_DISTANCE,
};
use crate::layout::{
    gate_visible_rect, layout_metrics, nearest_available_slot, nearest_line, nearest_slot_index,
    palette_hit_test, palette_layout, qft_resize_handle_rect, LayoutMetrics,
};

/// Column index the pointer is hovering over for the step-preview
/// interaction. Returns `None` when the cursor is outside the slot
/// row (above the top wire, below the bottom wire, or to the side of
/// the slot range). Steps map 1:1 to slots, so the index is the slot
/// the cursor would snap to if a gate were dropped here.
fn step_at_cursor(cursor: egui::Pos2, metrics: &LayoutMetrics) -> Option<usize> {
    if metrics.slot_centers.is_empty() || metrics.line_ys.is_empty() {
        return None;
    }
    let top = metrics.line_ys[0] - LINE_GAP * 0.5;
    let bottom = metrics.line_ys[metrics.line_ys.len() - 1] + LINE_GAP * 0.5;
    if cursor.y < top || cursor.y > bottom {
        return None;
    }
    if cursor.x < metrics.slot_left - SLOT_SPACING * 0.5
        || cursor.x > metrics.slot_right + SLOT_SPACING * 0.5
    {
        return None;
    }
    let (slot, dist) = nearest_slot_index(cursor.x, &metrics.slot_centers)?;
    if dist <= SLOT_SPACING * 0.5 {
        Some(slot)
    } else {
        None
    }
}
use crate::shared::now_seconds;

impl QniApp {
    pub(crate) fn handle_input(
        &mut self,
        content_rect: egui::Rect,
        ctx: &egui::Context,
        screen_rect: egui::Rect,
    ) {
        let pointer = ctx.input(|input| input.pointer.clone());
        let pos = pointer.latest_pos();
        let pointer_down = pointer.primary_down();
        let pointer_pressed = pointer.primary_pressed();
        let pointer_released = pointer.primary_released();

        let pointer_start = pointer_pressed || (pointer_down && !self.pointer_was_down);
        self.pointer_was_down = pointer_down;
        let local_pos = pos.map(|p| egui::pos2(p.x - content_rect.min.x, p.y - content_rect.min.y));
        let palette_geom = palette_layout();
        let palette_start_x = screen_rect.width() / 2.0 - palette_geom.total_width / 2.0;
        let palette_origin = egui::pos2(
            screen_rect.min.x + palette_start_x,
            screen_rect.min.y + PALETTE_ROW_Y,
        );
        let palette_rect = egui::Rect::from_min_size(
            palette_origin,
            egui::vec2(palette_geom.total_width, palette_geom.total_height),
        );
        let metrics = layout_metrics(content_rect.width(), self.layout_qubits());

        // QFT resize handle takes priority over gate body for press
        // events, so dragging the bottom-edge chevron resizes the span
        // instead of picking up the whole gate.
        if pointer_start {
            if let Some(cursor) = local_pos {
                if let Some((gate_id, start_span, start_pointer_y)) = self
                    .placed_gates
                    .iter()
                    .rev()
                    .find(|gate| {
                        if !gate.kind.is_resizable_span() {
                            return false;
                        }
                        let gate_rect = gate_visible_rect(gate, gate.pos);
                        qft_resize_handle_rect(gate_rect).contains(cursor)
                    })
                    .map(|gate| (gate.id, gate.span.max(1), cursor.y))
                {
                    self.qft_resize_drag = Some(QftResizeDrag {
                        gate_id,
                        start_pointer_y,
                        start_span,
                    });
                    self.hovered_gate_id = None;
                    self.hovered_palette_index = None;
                    ctx.request_repaint();
                    return;
                }
            }
        }

        // Active QFT resize → update span from total Δy and skip the
        // rest of the input pipeline (gate drag, hover) for this frame.
        if let Some(drag) = self.qft_resize_drag {
            if pointer_down || pointer_released {
                if let Some(cursor) = local_pos {
                    let delta_y = cursor.y - drag.start_pointer_y;
                    // One LINE_GAP of drag = one extra wire. Round to
                    // the nearest integer so the snap feels positive.
                    let span_delta = (delta_y / LINE_GAP).round() as i32;
                    let new_span = (drag.start_span as i32 + span_delta).clamp(1, QFT_MAX_SPAN as i32) as usize;
                    if let Some(index) =
                        self.placed_gates.iter().position(|g| g.id == drag.gate_id)
                    {
                        if self.placed_gates[index].span != new_span {
                            self.placed_gates[index].span = new_span;
                            self.update_qubit_count();
                            self.needs_recompute = true;
                            ctx.request_repaint();
                        }
                    }
                }
            }
            if pointer_released {
                self.qft_resize_drag = None;
                self.drag_repaint_deadline = None;
                self.drag_repaint_pending = false;
            }
            // Keep the cursor in a "resize" mode while the drag is active.
            ctx.set_cursor_icon(egui::CursorIcon::ResizeVertical);
            return;
        }

        if pointer_start {
            if let Some(cursor) = local_pos {
                if let Some((gate_id, offset)) = self
                    .placed_gates
                    .iter()
                    .rev()
                    .find(|gate| {
                        let gate_rect = gate_visible_rect(gate, gate.pos);
                        gate_rect.contains(cursor)
                    })
                    .map(|gate| (gate.id, cursor - gate.pos))
                {
                    self.dragging = Some(DragState {
                        id: gate_id,
                        offset,
                    });
                    self.drag_state_count = Some(self.state_count());
                    self.drag_cursor_pos = Some(cursor);
                    ctx.request_repaint();
                    self.hovered_gate_id = None;
                    self.hovered_palette_index = None;
                    return;
                }

                if let Some(cursor_screen) = pos {
                    let local = egui::pos2(
                        cursor_screen.x - palette_origin.x,
                        cursor_screen.y - palette_origin.y,
                    );
                    if let Some(index) = palette_hit_test(local, &palette_geom) {
                        let new_id = self.next_gate_id;
                        let new_gate = PlacedGate {
                            id: new_id,
                            kind: PALETTE_GATES[index],
                            pos: egui::pos2(
                                cursor.x - GATE_SIZE / 2.0,
                                cursor.y - GATE_SIZE / 2.0,
                            ),
                            wire: 0,
                            span: 1,
                        };
                        self.next_gate_id += 1;
                        self.placed_gates.push(new_gate);
                        self.dragging = Some(DragState {
                            id: new_id,
                            offset: egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0),
                        });
                        self.drag_state_count = Some(self.state_count());
                        self.drag_cursor_pos = Some(cursor);
                        ctx.request_repaint();
                        self.hovered_palette_index = None;
                        self.hovered_gate_id = None;
                        return;
                    }
                }

                // No gate / palette under the cursor. If we're inside a
                // step slot, lock the breakpoint to that column.
                if let Some(step) = step_at_cursor(cursor, &metrics) {
                    if self.breakpoint_step != Some(step) {
                        self.breakpoint_step = Some(step);
                        self.needs_recompute = true;
                        ctx.request_repaint();
                    }
                    return;
                }
            }
        }

        if let Some(drag) = self.dragging.as_ref() {
            if pointer_down || pointer_released {
                let cursor = local_pos.or(self.drag_cursor_pos);
                if let Some(cursor) = cursor {
                    self.drag_cursor_pos = Some(cursor);
                    if let Some(index) =
                        self.placed_gates.iter().position(|gate| gate.id == drag.id)
                    {
                        let mut next_pos = cursor - drag.offset;
                        let mut next_wire = self.placed_gates[index].wire;
                        let center_y = next_pos.y + GATE_SIZE / 2.0;
                        let (line_y, distance, line_index) =
                            nearest_line(center_y, &metrics.line_ys);
                        if distance <= SNAP_DISTANCE {
                            next_pos.y = line_y - GATE_SIZE / 2.0;
                            next_wire = line_index;
                            let center_x = next_pos.x + GATE_SIZE / 2.0;
                            if let Some((slot_center, _)) = nearest_available_slot(
                                center_x,
                                line_index,
                                Some(drag.id),
                                &self.placed_gates,
                                &metrics.slot_centers,
                            ) {
                                next_pos.x = slot_center - GATE_SIZE / 2.0;
                            }
                        }
                        let gate = &mut self.placed_gates[index];
                        gate.pos = next_pos;
                        gate.wire = next_wire;
                    }
                }
            }
        } else if let Some(cursor) = local_pos {
            // Iterate top-of-stack first so the QFT resize handle wins
            // over the gate body when the cursor is on both (the handle
            // overhangs the gate's bottom edge).
            let mut hovered_gate = None;
            let mut hovered_handle = None;
            for gate in self.placed_gates.iter().rev() {
                let gate_rect = gate_visible_rect(gate, gate.pos);
                if gate.kind.is_resizable_span()
                    && qft_resize_handle_rect(gate_rect).contains(cursor)
                {
                    hovered_handle = Some(gate.id);
                    hovered_gate = Some(gate.id);
                    break;
                }
                if gate_rect.contains(cursor) {
                    hovered_gate = Some(gate.id);
                    break;
                }
            }
            self.hovered_gate_id = hovered_gate;
            self.hovered_qft_resize_handle = hovered_handle;

            // Step preview: which column is the cursor on? Changes
            // trigger a recompute so the state-vector panel reflects
            // the new step in real time.
            let new_hovered_step = step_at_cursor(cursor, &metrics);
            if new_hovered_step != self.hovered_step {
                self.hovered_step = new_hovered_step;
                self.needs_recompute = true;
                ctx.request_repaint();
            }

            let mut hovered_palette = None;
            if let Some(cursor_screen) = pos {
                if palette_rect.contains(cursor_screen) {
                    let local = egui::pos2(
                        cursor_screen.x - palette_origin.x,
                        cursor_screen.y - palette_origin.y,
                    );
                    hovered_palette = palette_hit_test(local, &palette_geom);
                }
            }
            self.hovered_palette_index = hovered_palette;
        } else {
            self.hovered_gate_id = None;
            self.hovered_qft_resize_handle = None;
            self.hovered_palette_index = None;
            if self.hovered_step.is_some() {
                self.hovered_step = None;
                self.needs_recompute = true;
            }
        }

        if pointer_released {
            if let Some(drag) = self.dragging.take() {
                if let Some(index) = self.placed_gates.iter().position(|gate| gate.id == drag.id) {
                    let gate_pos = self.placed_gates[index].pos;
                    let gate_id = self.placed_gates[index].id;
                    let center_x = gate_pos.x + GATE_SIZE / 2.0;
                    let center_y = gate_pos.y + GATE_SIZE / 2.0;
                    let (line_y, distance, line_index) = nearest_line(center_y, &metrics.line_ys);
                    let snapped = nearest_available_slot(
                        center_x,
                        line_index,
                        Some(gate_id),
                        &self.placed_gates,
                        &metrics.slot_centers,
                    );
                    let on_circuit = center_x >= metrics.slot_left
                        && center_x <= metrics.slot_right
                        && distance <= SNAP_DISTANCE
                        && snapped.map(|(_, d)| d <= SNAP_DISTANCE).unwrap_or(false);

                    if !on_circuit {
                        self.placed_gates.remove(index);
                    } else if let Some((slot_center, _)) = snapped {
                        let gate = &mut self.placed_gates[index];
                        gate.pos.x = slot_center - GATE_SIZE / 2.0;
                        gate.pos.y = line_y - GATE_SIZE / 2.0;
                        gate.wire = line_index;
                    }
                    // Mirror qni's post-drop `resize()` pipeline: any
                    // column that became empty (either because the
                    // user moved a gate off the circuit, or because
                    // they dropped one past an empty step) is removed
                    // and the trailing gates shift left to fill the
                    // gap. Runs for both branches above.
                    self.compact_empty_steps(&metrics.slot_centers);
                    self.update_qubit_count();
                    self.needs_recompute = true;
                    ctx.request_repaint();
                }
            }
            self.drag_state_count = None;
            self.drag_repaint_deadline = None;
            self.drag_repaint_pending = false;
            self.drag_cursor_pos = None;
        }

        if self.dragging.is_some() && pointer_down {
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if self.hovered_qft_resize_handle.is_some() {
            ctx.set_cursor_icon(egui::CursorIcon::ResizeVertical);
        } else if self.hovered_gate_id.is_some() || self.hovered_palette_index.is_some() {
            ctx.set_cursor_icon(egui::CursorIcon::Grab);
        }
    }

    pub(crate) fn schedule_drag_repaint(&mut self, ctx: &egui::Context, frame_secs: f64) {
        let now = now_seconds();
        let deadline = self.drag_repaint_deadline.unwrap_or(now);
        if now >= deadline {
            let delay = (DRAG_REPAINT_BASE_SECS + frame_secs * DRAG_REPAINT_PUMP_FACTOR)
                .clamp(DRAG_REPAINT_MIN_SECS, DRAG_REPAINT_MAX_SECS);
            self.drag_repaint_deadline = Some(now + delay);
            self.drag_repaint_pending = false;
            ctx.request_repaint();
        } else if !self.drag_repaint_pending {
            self.drag_repaint_pending = true;
            let remaining = (deadline - now).max(0.0);
            ctx.request_repaint_after(Duration::from_secs_f64(remaining));
        }
    }
}
