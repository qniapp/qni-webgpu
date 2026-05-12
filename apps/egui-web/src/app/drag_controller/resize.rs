use eframe::egui;

use super::{reset_drag_frame_state, DragController, DragPointer};
use crate::app::QniApp;
use crate::constants::{LINE_GAP, QFT_MAX_SPAN};

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
                let new_span =
                    (drag.start_span as i32 + span_delta).clamp(1, QFT_MAX_SPAN as i32) as usize;
                if let Some(index) = app
                    .placed_gates
                    .iter()
                    .position(|gate| gate.id == drag.gate_id)
                {
                    if app.placed_gates[index].span != new_span {
                        app.placed_gates[index].span = new_span;
                        app.update_qubit_count();
                        app.gpu_plan.mark_dirty();
                        ctx.request_repaint();
                    }
                }
            }
        }
        if pointer.released {
            app.qft_resize_drag = None;
            reset_drag_frame_state(app);
        }
        // Keep the cursor in a "resize" mode while the drag is active.
        ctx.set_cursor_icon(egui::CursorIcon::ResizeVertical);
        true
    }
}
