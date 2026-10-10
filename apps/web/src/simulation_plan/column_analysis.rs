//! Shared column grouping for simulation lowering and connector painting.
//!
//! A column is identified by a caller-provided slot key: semantic columns use
//! the editor column index, while render columns use the snap/drag-aware slot
//! index. Keeping the grouping algorithm here prevents GPU lowering and visual
//! connector code from growing separate HashMap/sort semantics.

use std::collections::BTreeMap;

use crate::app::PlacedGate;
use crate::gates::{ColumnControls, GateFlag, GateKind, ParametricAngle};
use crate::qubit_count::QubitCount;

/// Which multi-controlled phase (CPHASE / CCPHASE) a `P` gate belongs to.
///
/// Same-column `P` gates with an equal key act as one operation: `e^{iφ}`
/// applies only when every one of their wires reads |1⟩ (plus the column's
/// `•` / `◦` conditions). The key mirrors qni's step serialization
/// (`circuit-step-element.ts` :1095-1112), which groups phase gates by angle
/// and then by `if` into one `{type: phase, targets, angle}`. A bare `P`
/// takes the editor's π/2 default, so it groups with `P(π_2)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct PhaseGroupKey<'a> {
    angle: ParametricAngle,
    flag: Option<&'a GateFlag>,
}

impl<'a> PhaseGroupKey<'a> {
    /// `None` when `gate` is not a `P` gate.
    pub(crate) fn of(gate: &'a PlacedGate) -> Option<Self> {
        (gate.kind == GateKind::Phase).then(|| Self {
            angle: gate.angle.unwrap_or_default(),
            flag: gate.flag.as_ref(),
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct AnalyzedColumn<'a> {
    pub(crate) slot: usize,
    gates: Vec<&'a PlacedGate>,
}

impl<'a> AnalyzedColumn<'a> {
    pub(crate) fn gates(&self) -> &[&'a PlacedGate] {
        &self.gates
    }
}

/// Only semantic columns have a computed control mask.
pub(crate) struct SimulationColumn<'a> {
    pub(crate) slot: usize,
    gates: Vec<&'a PlacedGate>,
    controls: ColumnControls,
}

impl<'a> SimulationColumn<'a> {
    pub(crate) fn gates(&self) -> &[&'a PlacedGate] {
        &self.gates
    }

    pub(crate) fn controls(&self) -> ColumnControls {
        self.controls
    }

    pub(crate) fn displays(&self, kind: GateKind) -> Vec<&'a PlacedGate> {
        let mut displays: Vec<_> = self
            .gates
            .iter()
            .copied()
            .filter(|gate| gate.kind == kind)
            .collect();
        displays.sort_by_key(|gate| gate.id);
        displays
    }
}

pub(crate) struct SimulationColumnAnalysis<'a> {
    columns: Vec<SimulationColumn<'a>>,
}

impl<'a> SimulationColumnAnalysis<'a> {
    pub(crate) fn from_gates(placed_gates: &'a [PlacedGate], qubits: QubitCount) -> Self {
        let grouped = ColumnAnalysis::from_gates(placed_gates, |gate| {
            gate.wire.is_within(qubits).then(|| gate.column.as_usize())
        });
        let columns = grouped
            .columns
            .into_iter()
            .map(|column| {
                let mut controls = ColumnControls::NONE;
                for gate in &column.gates {
                    let bit = gate
                        .wire
                        .to_qubit_bit(qubits)
                        .expect("simulation column wires are within the register");
                    match gate.kind {
                        GateKind::Control => controls.add_control(bit),
                        GateKind::AntiControl => controls.add_anti_control(bit),
                        _ => {}
                    }
                }
                SimulationColumn {
                    slot: column.slot,
                    gates: column.gates,
                    controls,
                }
            })
            .collect();
        Self { columns }
    }

    pub(crate) fn columns(&self) -> &[SimulationColumn<'a>] {
        &self.columns
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ColumnAnalysis<'a> {
    columns: Vec<AnalyzedColumn<'a>>,
}

impl<'a> ColumnAnalysis<'a> {
    pub(crate) fn from_gates(
        placed_gates: &'a [PlacedGate],
        mut slot_for_gate: impl FnMut(&'a PlacedGate) -> Option<usize>,
    ) -> Self {
        let mut by_slot: BTreeMap<usize, Vec<&'a PlacedGate>> = BTreeMap::new();
        for gate in placed_gates {
            let Some(slot) = slot_for_gate(gate) else {
                continue;
            };
            by_slot.entry(slot).or_default().push(gate);
        }
        let columns = by_slot
            .into_iter()
            .map(|(slot, gates)| AnalyzedColumn { slot, gates })
            .collect();
        Self { columns }
    }

    pub(crate) fn columns(&self) -> &[AnalyzedColumn<'a>] {
        &self.columns
    }
}
