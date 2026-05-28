use eframe::egui;

use super::{reset_drag_frame_state, DragController, DragPointer};
use crate::app::QniApp;
use crate::app::SpanResizeEdge;
use crate::constants::LINE_GAP;
use crate::layout::gate_width_cols;

impl DragController {
    pub(in crate::app) fn update_active_span_resize(
        app: &mut QniApp,
        pointer: DragPointer,
        ctx: &egui::Context,
    ) -> bool {
        let Some(drag) = app.span_resize_drag else {
            return false;
        };
        if pointer.down || pointer.released {
            if let Some(cursor) = pointer.local_pos {
                let delta_y = cursor.y - drag.start_pointer_y;
                // One LINE_GAP of drag = one extra wire. Round to the
                // nearest integer so the snap feels positive.
                let span_delta = (delta_y / LINE_GAP).round() as i32;
                if let Some(index) = app
                    .placed_gates
                    .iter()
                    .position(|gate| gate.id == drag.gate_id)
                {
                    let capacity = app.exec_mode.qubit_capacity();
                    let gate = &mut app.placed_gates[index];
                    let gate_id = gate.id;
                    let gate_kind = gate.kind;
                    let gate_column = gate.column;
                    let old_wire = gate.wire;
                    let old_span = gate.span;
                    let old_width = gate_width_cols(gate_kind, old_span);
                    match drag.edge {
                        SpanResizeEdge::Bottom => {
                            let remaining_wires = capacity.get().saturating_sub(gate.wire).max(1);
                            let max_span = gate.kind.max_resizable_span(remaining_wires);
                            gate.span = (drag.start_span as i32 + span_delta)
                                .clamp(1, max_span as i32)
                                as usize;
                        }
                        SpanResizeEdge::Top => {
                            let bottom_wire = drag.start_wire + drag.start_span.saturating_sub(1);
                            let max_span = gate.kind.max_resizable_span(bottom_wire + 1);
                            let min_wire = bottom_wire + 1 - max_span;
                            let new_wire = (drag.start_wire as i32 + span_delta)
                                .clamp(min_wire as i32, bottom_wire as i32)
                                as usize;
                            gate.wire = new_wire;
                            gate.span = bottom_wire - new_wire + 1;
                            gate.sync_pos_from_grid();
                        }
                    }
                    gate.clamp_span_to_qubit_capacity(capacity);
                    let new_wire = gate.wire;
                    let new_span = gate.span;
                    let new_width = gate_width_cols(gate_kind, new_span);
                    let changed = new_wire != old_wire || new_span != old_span;
                    if changed {
                        app.shift_trailing_gates_after_width_change(
                            gate_id,
                            gate_column,
                            old_width,
                            new_width,
                        );
                        app.update_qubit_count();
                        app.gpu_plan.mark_dirty();
                        ctx.request_repaint();
                    }
                }
            }
        }
        if pointer.released {
            app.span_resize_drag = None;
            app.commit_current_circuit(ctx);
            reset_drag_frame_state(app);
        }
        // Keep the cursor in a "resize" mode while the drag is active.
        ctx.set_cursor_icon(egui::CursorIcon::ResizeVertical);
        true
    }
}
