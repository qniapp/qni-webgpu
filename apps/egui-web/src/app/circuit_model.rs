//! Circuit-domain model helpers extracted from `QniApp`.
//!
//! This module keeps qni's semantic step/dropzone idea explicit: committed
//! gates are addressed by `column` + `wire`; pixels are derived layout data or
//! drag previews, never the source of truth for URL / GPU planning.

use eframe::egui;
use std::collections::{BTreeSet, HashMap};

use crate::constants::{
    GATE_SIZE, LINE_GAP, LINE_LEFT_OFFSET, LINE_Y, MAX_QUBITS, MIN_QUBITS, SLOT_SPACING,
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
    /// QFT / QFT† can be resized to span 2+ wires via the bottom-edge
    /// resize handle that appears on hover.
    pub(crate) span: usize,
    /// Angle string for parametric gates (currently only `GateKind::Phase`).
    /// Stored as the raw qni-compatible expression — e.g. `"π/2"`, `"-π/128"`,
    /// `"2π/3"`, `"0"` — so URL round-trips are exact. `None` means
    /// "use the gate's default" (palette-placed Phase falls back to π/2
    /// to preserve the editor's pre-parametric behaviour); qni would
    /// instead error out at simulate time.
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
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DragState {
    pub(crate) id: u32,
    pub(crate) offset: egui::Vec2,
}

/// In-flight resize of a QFT-family gate's vertical span. Tracks which gate's
/// resize handle was grabbed and the start span so per-frame drag math derives
/// the new span from the *total* cursor delta.
#[derive(Clone, Copy, Debug)]
pub(crate) struct QftResizeDrag {
    pub(crate) gate_id: u32,
    pub(crate) start_pointer_y: f32,
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
            .map(|gate| gate.column + 2)
            .max()
            .unwrap_or(0)
    }

    pub(super) fn state_qubits(&self) -> usize {
        let mut max_wire: Option<usize> = None;
        for gate in &self.placed_gates {
            let bottom = gate.wire + gate.span.saturating_sub(1);
            max_wire = Some(match max_wire {
                Some(current) => current.max(bottom),
                None => bottom,
            });
        }
        let count = max_wire.map_or(1, |wire| wire + 1);
        count.clamp(1, MAX_QUBITS)
    }

    pub(super) fn update_qubit_count(&mut self) {
        let mut max_wire = MIN_QUBITS - 1;
        for gate in &self.placed_gates {
            // A multi-qubit gate at `wire` with `span = N` occupies wires
            // [wire, wire + N - 1]; the bottom of that range bounds the qubit
            // count.
            let bottom_wire = gate.wire + gate.span.saturating_sub(1);
            max_wire = max_wire.max(bottom_wire);
        }
        self.qubit_count = (max_wire + 1).clamp(MIN_QUBITS, MAX_QUBITS);
    }

    /// After a successful drop or off-circuit removal, collapse empty columns
    /// and shift trailing gates left. Mirrors qni's
    /// `QuantumCircuitElement.removeEmptySteps()`.
    pub(crate) fn compact_empty_steps(&mut self) {
        if self.placed_gates.is_empty() {
            return;
        }
        let occupied: BTreeSet<usize> = self.placed_gates.iter().map(|gate| gate.column).collect();
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

    pub(super) fn state_count(&self) -> usize {
        1usize << self.state_qubits()
    }
}
