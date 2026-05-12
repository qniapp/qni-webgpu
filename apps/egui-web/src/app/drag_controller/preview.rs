use super::{DragController, DragPointer};
use crate::app::QniApp;
use crate::constants::{GATE_SIZE, SNAP_DISTANCE};
use crate::layout::{nearest_circuit_snap, nearest_line, LayoutMetrics};

impl DragController {
    pub(in crate::app) fn update_gate_drag_preview(
        app: &mut QniApp,
        pointer: DragPointer,
        metrics: &LayoutMetrics,
    ) {
        let Some(drag) = app.dragging else {
            return;
        };
        if !(pointer.down || pointer.released) {
            return;
        }
        let cursor = pointer.local_pos.or(app.drag_cursor_pos);
        let Some(cursor) = cursor else {
            return;
        };
        app.drag_cursor_pos = Some(cursor);
        let Some(index) = app.placed_gates.iter().position(|gate| gate.id == drag.id) else {
            return;
        };

        let mut next_pos = cursor - drag.offset;
        let mut next_wire = app.placed_gates[index].wire;
        let mut next_column = app.placed_gates[index].column;
        let center_y = next_pos.y + GATE_SIZE / 2.0;
        let (line_y, distance, line_index) = nearest_line(center_y, &metrics.line_ys);
        if distance <= SNAP_DISTANCE {
            next_pos.y = line_y - GATE_SIZE / 2.0;
            next_wire = line_index;
            let center_x = next_pos.x + GATE_SIZE / 2.0;
            if let Some(snap) = nearest_circuit_snap(
                center_x,
                line_index,
                Some(drag.id),
                &app.placed_gates,
                &metrics.slot_centers,
            ) {
                next_pos.x = snap.center() - GATE_SIZE / 2.0;
                next_column = snap.column();
            }
        }
        let gate = &mut app.placed_gates[index];
        gate.pos = next_pos;
        gate.wire = next_wire;
        gate.column = next_column;
    }
}
