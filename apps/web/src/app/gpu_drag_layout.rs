//! Tentative insertion operands for GPU planning only.
//! Drawing, pointer snapping, and history use the uncommitted editor model.

use std::borrow::Cow;

use super::circuit_model::{compact_gate_columns, insert_gate_in};
use super::{LiveDragSnap, PlacedGate, QniApp};

impl QniApp {
    pub(crate) fn gpu_plan_gates(&self) -> Cow<'_, [PlacedGate]> {
        let Some(drag) = self.dragging else {
            return Cow::Borrowed(&self.placed_gates);
        };
        let Some(LiveDragSnap::Insert { column, wire }) = self.dragging_live_snap else {
            return Cow::Borrowed(&self.placed_gates);
        };
        let mut gates = self.placed_gates.clone();
        insert_gate_in(
            &mut gates,
            drag.id,
            wire,
            column,
            drag.original_column,
            self.wire_capacity(),
        );
        compact_gate_columns(&mut gates);
        Cow::Owned(gates)
    }
}
