use eframe::egui;

use super::{reset_drag_frame_state, DragController, DragPointer};
use crate::app::QniApp;
use crate::constants::LINE_GAP;

impl DragController {
    pub(in crate::app) fn update_active_qft_resize(
        app: &mut QniApp,
        pointer: DragPointer,
        ctx: &egui::Context,
    ) -> bool {
        let Some(drag) = app.qft_resize_drag else {
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
                    let max_span = capacity.saturating_sub(app.placed_gates[index].wire).max(1);
                    let new_span =
                        (drag.start_span as i32 + span_delta).clamp(1, max_span as i32) as usize;
                    if app.placed_gates[index].span != new_span {
                        app.placed_gates[index].span = new_span;
                        app.placed_gates[index].clamp_span_to_qubit_capacity(capacity);
                        app.update_qubit_count();
                        app.gpu_plan.mark_dirty();
                        ctx.request_repaint();
                    }
                }
            }
        }
        if pointer.released {
            app.qft_resize_drag = None;
            app.commit_current_circuit(ctx);
            reset_drag_frame_state(app);
        }
        // Keep the cursor in a "resize" mode while the drag is active.
        ctx.set_cursor_icon(egui::CursorIcon::ResizeVertical);
        true
    }
}
