//! Circuit-domain model helpers extracted from `QniApp`.
//!
//! This module keeps qni's semantic step/dropzone idea explicit: committed
//! gates are addressed by `column` + `wire`; pixels are derived layout data or
//! drag previews, never the source of truth for URL / GPU planning.

use eframe::egui;
use std::collections::{BTreeSet, HashMap};

use crate::layout::gate_width_cols;

use crate::constants::{
    GATE_SIZE, LINE_GAP, LINE_LEFT_OFFSET, LINE_Y, LOCAL_MAX_QUBITS, MIN_QUBITS, SLOT_SPACING,
};
use crate::gates::GateKind;

use super::QniApp;

#[derive(Clone, Debug)]
pub(crate) struct PlacedGate {
    pub(crate) id: u32,
    pub(crate) kind: GateKind,
    /// Semantic circuit column (qni `CircuitStepElement` index). This is the
    /// authoritative horizontal model; `pos.x` is only the derived draw/drag
    /// preview coordinate.
    pub(crate) column: usize,
    /// Derived circuit-local draw position. During drag this follows the
    /// pointer as a preview; on committed placement it is resynchronised from
    /// `column` / `wire`.
    pub(crate) pos: egui::Pos2,
    pub(crate) wire: usize,
    /// Vertical span in qubit wires. 1 for ordinary single-qubit gates;
    /// resizable-span gates (Probability, Amplitude, QFT / QFT†) can grow via hover-revealed
    /// resize handles.
    pub(crate) span: usize,
    /// Angle string for parametric gates (currently only `GateKind::Phase`).
    /// Stored as the raw qni-compatible expression — e.g. `"π/2"`, `"-π/128"`,
    /// `"2π/3"`, `"0"` — so URL round-trips are exact. Palette-placed Phase
    /// gates store the explicit default `"π/2"` so the circuit can show the
    /// angle label immediately. `None` still represents a bare legacy token and
    /// is evaluated as the gate's default.
    pub(crate) angle: Option<String>,
}

impl PlacedGate {
    pub(crate) fn new(
        id: u32,
        kind: GateKind,
        column: usize,
        wire: usize,
        span: usize,
        angle: Option<String>,
    ) -> Self {
        Self {
            id,
            kind,
            column,
            pos: Self::grid_pos(column, wire),
            wire,
            span,
            angle,
        }
    }

    pub(crate) fn grid_pos(column: usize, wire: usize) -> egui::Pos2 {
        let slot_left = LINE_LEFT_OFFSET + GATE_SIZE;
        let slot_center_x = slot_left + SLOT_SPACING * column as f32;
        let line_y = LINE_Y + LINE_GAP * wire as f32;
        egui::pos2(slot_center_x - GATE_SIZE / 2.0, line_y - GATE_SIZE / 2.0)
    }

    pub(crate) fn sync_pos_from_grid(&mut self) {
        self.pos = Self::grid_pos(self.column, self.wire);
    }

    pub(crate) fn clamp_span_to_qubit_capacity(&mut self, capacity: usize) {
        if !self.kind.is_resizable_span() {
            return;
        }
        let remaining_wires = capacity.saturating_sub(self.wire).max(1);
        let max_span = self.kind.max_resizable_span(remaining_wires);
        self.span = self.span.clamp(1, max_span);
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DragState {
    pub(crate) id: u32,
    pub(crate) offset: egui::Vec2,
    /// Original semantic column for an existing gate. `None` means a palette
    /// gate that has no committed source column yet.
    pub(crate) original_column: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LiveDragSnap {
    Slot { column: usize, wire: usize },
    Insert { column: usize, wire: usize },
}

/// Which vertical edge of a span-resizable gate is being manipulated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpanResizeEdge {
    Top,
    Bottom,
}

/// A concrete resizable-span handle under the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SpanResizeHandle {
    pub(crate) gate_id: u32,
    pub(crate) edge: SpanResizeEdge,
}

/// In-flight resize of a resizable-span gate's vertical span. Tracks which
/// handle was grabbed and the starting grid geometry so per-frame drag math
/// derives the new span from the *total* cursor delta.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SpanResizeDrag {
    pub(crate) gate_id: u32,
    pub(crate) edge: SpanResizeEdge,
    pub(crate) start_pointer_y: f32,
    pub(crate) start_wire: usize,
    pub(crate) start_span: usize,
}

impl QniApp {
    /// Minimum number of slot centers the layout must expose so every placed
    /// gate has a valid snap target. Passed to `layout_metrics` so the wire
    /// stretches all the way past the rightmost gate even when that gate sits
    /// beyond the canvas's natural right edge.
    ///
    /// Each gate's column is semantic state (qni's step index), not a value
    /// recovered from pixels. Reserve one extra trailing slot as a drop-target
    /// landing zone (mirrors qni's `appendMinimumSteps`).
    pub(super) fn min_circuit_slots(&self) -> usize {
        self.placed_gates
            .iter()
            .map(|gate| gate.column + gate_width_cols(gate.kind, gate.span) + 1)
            .max()
            .unwrap_or(0)
    }

    fn raw_required_qubit_count(&self) -> usize {
        self.placed_gates
            .iter()
            .map(|gate| gate.wire + gate.span.saturating_sub(1) + 1)
            .max()
            .unwrap_or(0)
    }

    pub(super) fn required_qubit_count(&self) -> usize {
        self.raw_required_qubit_count().max(MIN_QUBITS)
    }

    pub(super) fn external_execution_qubits(&self) -> usize {
        self.raw_required_qubit_count()
            .max(1)
            .min(self.exec_mode.qubit_capacity())
    }

    pub(super) fn state_qubits(&self) -> usize {
        self.raw_required_qubit_count()
            .max(1)
            .clamp(1, LOCAL_MAX_QUBITS)
    }

    pub(crate) fn update_qubit_count(&mut self) {
        self.qubit_count = self
            .required_qubit_count()
            .clamp(MIN_QUBITS, self.exec_mode.qubit_capacity());
    }

    /// After a successful drop or off-circuit removal, collapse empty columns
    /// and shift trailing gates left. Mirrors qni's
    /// `QuantumCircuitElement.removeEmptySteps()`.
    pub(crate) fn compact_empty_steps(&mut self) {
        if self.placed_gates.is_empty() {
            return;
        }
        let occupied: BTreeSet<usize> = self
            .placed_gates
            .iter()
            .flat_map(|gate| gate.column..gate.column + gate_width_cols(gate.kind, gate.span))
            .collect();
        let already_compact = occupied
            .iter()
            .enumerate()
            .all(|(new_i, &old_i)| new_i == old_i);
        if already_compact {
            return;
        }
        let mut remap: HashMap<usize, usize> = HashMap::with_capacity(occupied.len());
        for (new_i, &old_i) in occupied.iter().enumerate() {
            remap.insert(old_i, new_i);
        }
        for gate in &mut self.placed_gates {
            if let Some(&new_i) = remap.get(&gate.column) {
                gate.column = new_i;
                gate.sync_pos_from_grid();
            }
        }
    }

    /// After a resizable gate changes horizontal footprint, move every gate
    /// that starts at or after the old right edge by the width delta so the
    /// grown display does not overlap later columns.
    pub(crate) fn shift_trailing_gates_after_width_change(
        &mut self,
        gate_id: u32,
        column: usize,
        old_width: usize,
        new_width: usize,
    ) {
        if old_width == new_width {
            return;
        }
        let boundary = column + old_width;
        if new_width > old_width {
            let delta = new_width - old_width;
            for gate in &mut self.placed_gates {
                if gate.id != gate_id && gate.column >= boundary {
                    gate.column += delta;
                    gate.sync_pos_from_grid();
                }
            }
        } else {
            let delta = old_width - new_width;
            for gate in &mut self.placed_gates {
                if gate.id != gate_id && gate.column >= boundary {
                    gate.column = gate.column.saturating_sub(delta);
                    gate.sync_pos_from_grid();
                }
            }
        }
    }

    pub(crate) fn insert_gate_at_column(
        &mut self,
        gate_id: u32,
        wire: usize,
        insert_index: usize,
        original_column: Option<usize>,
    ) {
        let Some(gate_index) = self.placed_gates.iter().position(|gate| gate.id == gate_id) else {
            return;
        };

        let moving_width = gate_width_cols(
            self.placed_gates[gate_index].kind,
            self.placed_gates[gate_index].span,
        );
        let mut adjusted_insert = insert_index;
        if let Some(old_column) = original_column {
            let old_width = moving_width;
            let old_column_still_occupied = self
                .placed_gates
                .iter()
                .any(|gate| gate.id != gate_id && gate.column == old_column);
            if !old_column_still_occupied {
                for gate in &mut self.placed_gates {
                    if gate.id != gate_id && gate.column > old_column {
                        gate.column = gate.column.saturating_sub(old_width);
                    }
                }
                if old_column < adjusted_insert {
                    adjusted_insert = adjusted_insert.saturating_sub(old_width);
                }
            }
        }

        for gate in &mut self.placed_gates {
            if gate.id != gate_id && gate.column >= adjusted_insert {
                gate.column += moving_width;
            }
        }

        let capacity = self.exec_mode.qubit_capacity();
        let gate = &mut self.placed_gates[gate_index];
        gate.column = adjusted_insert;
        gate.wire = wire;
        gate.clamp_span_to_qubit_capacity(capacity);

        for gate in &mut self.placed_gates {
            gate.sync_pos_from_grid();
        }
    }

    pub(super) fn state_count(&self) -> usize {
        1usize << self.state_qubits()
    }
}
