use eframe::egui;

use super::{reset_drag_frame_state, DragController, DragPointer};
use crate::app::QniApp;
use crate::constants::{GATE_SIZE, SNAP_DISTANCE};
use crate::layout::{nearest_available_slot, nearest_line, LayoutMetrics};

impl DragController {
    pub(in crate::app) fn commit_gate_drop(
        app: &mut QniApp,
        pointer: DragPointer,
        metrics: &LayoutMetrics,
        ctx: &egui::Context,
    ) {
        if !pointer.released {
            return;
        }
        if let Some(drag) = app.dragging.take() {
            if let Some(index) = app.placed_gates.iter().position(|gate| gate.id == drag.id) {
                let gate_pos = app.placed_gates[index].pos;
                let gate_id = app.placed_gates[index].id;
                let center_x = gate_pos.x + GATE_SIZE / 2.0;
                let center_y = gate_pos.y + GATE_SIZE / 2.0;
                let (_line_y, distance, line_index) = nearest_line(center_y, &metrics.line_ys);
                let snapped = nearest_available_slot(
                    center_x,
                    line_index,
                    Some(gate_id),
                    &app.placed_gates,
                    &metrics.slot_centers,
                );
                let on_circuit = center_x >= metrics.slot_left
                    && center_x <= metrics.slot_right
                    && distance <= SNAP_DISTANCE
                    && snapped
                        .as_ref()
                        .map(|snap| snap.distance <= SNAP_DISTANCE)
                        .unwrap_or(false);

                if !on_circuit {
                    app.placed_gates.remove(index);
                } else if let Some(snap) = snapped {
                    let gate = &mut app.placed_gates[index];
                    gate.column = snap.index;
                    gate.wire = line_index;
                    gate.sync_pos_from_grid();
                }
                // Mirror qni's post-drop `resize()`: remove empty
                // columns and shift trailing gates left for both branches.
                app.compact_empty_steps();
                app.update_qubit_count();
                // Mirror qni / Quirk: every committed circuit change
                // syncs to the URL hash. We use Quirk's readable-JSON
                // format (`#circuit={"cols":[...]}`) instead of qni's
                // percent-encoded path.
                let json = crate::url_circuit::circuit_to_json(&app.placed_gates, app.qubit_count);
                crate::url_circuit::write_circuit_to_url(&json);
                app.gpu_plan.mark_dirty();
                ctx.request_repaint();
            }
        }
        app.drag_state_count = None;
        reset_drag_frame_state(app);
        app.drag_cursor_pos = None;
    }
}
