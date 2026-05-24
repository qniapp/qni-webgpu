use super::{DragController, DragPointer};
use crate::app::QniApp;
use crate::constants::{GATE_SIZE, SNAP_DISTANCE};
use crate::gates::GateKind;
use crate::layout::{nearest_circuit_snap, nearest_line, CircuitSnap, LayoutMetrics};

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
        let previous_snapped = app.dragging_live_display_snap;
        let previous_wire = app.placed_gates[index].wire;
        let previous_column = app.placed_gates[index].column;
        let previous_span = app.placed_gates[index].span;

        let mut next_pos = cursor - drag.offset;
        let mut next_wire = previous_wire;
        let mut next_column = previous_column;
        let mut snapped_to_live_display_slot = false;
        let center_y = next_pos.y + GATE_SIZE / 2.0;
        let (line_y, distance, line_index) = nearest_line(center_y, &metrics.line_ys);
        if distance <= SNAP_DISTANCE {
            next_pos.y = line_y - GATE_SIZE / 2.0;
            next_wire = line_index;
            let center_x = next_pos.x + GATE_SIZE / 2.0;
            // Keep horizontal snapping sticky, not magnetic: outside the
            // snap radius the dragged gate must keep following the pointer
            // so dropping left/right of the circuit can remove it.
            if let Some(snap) = nearest_circuit_snap(
                center_x,
                line_index,
                &app.placed_gates[index],
                Some(drag.id),
                &app.placed_gates,
                &metrics.slot_centers,
            )
            .filter(|snap| snap.distance() <= SNAP_DISTANCE)
            {
                next_pos.x = snap.center() - GATE_SIZE / 2.0;
                next_column = snap.column();
                snapped_to_live_display_slot = matches!(snap, CircuitSnap::Slot(_));
            }
        }
        let capacity = app.exec_mode.qubit_capacity();
        let (gate_kind, gate_wire, gate_column, gate_span) = {
            let gate = &mut app.placed_gates[index];
            gate.pos = next_pos;
            gate.wire = next_wire;
            gate.column = next_column;
            gate.clamp_span_to_qubit_capacity(capacity);
            (gate.kind, gate.wire, gate.column, gate.span)
        };
        let live_display_kind = matches!(
            gate_kind,
            GateKind::AmplitudeDisplay
                | GateKind::DensityMatrixDisplay
                | GateKind::BlochDisplay
                | GateKind::Measurement
        );
        app.dragging_live_display_snap = live_display_kind && snapped_to_live_display_slot;
        let state_count_changed = if app.dragging_live_display_snap {
            let required_state_count = app.state_count();
            if app
                .drag_state_count
                .is_some_and(|state_count| state_count < required_state_count)
            {
                app.drag_state_count = Some(required_state_count);
                true
            } else {
                false
            }
        } else {
            false
        };
        if app.dragging_live_display_snap
            && (!previous_snapped
                || previous_wire != gate_wire
                || previous_column != gate_column
                || previous_span != gate_span
                || state_count_changed)
        {
            // A snapped display drag has a meaningful tentative placement.
            // Rebuild the GPU capture plan so the drag preview can render live
            // GPU values instead of the static placeholder.
            app.gpu_plan.mark_dirty();
        }
    }
}
