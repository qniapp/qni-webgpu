//! Shared column grouping for simulation lowering and connector painting.
//!
//! A column is identified by a caller-provided slot key: semantic columns use
//! the editor column index, while render columns use the snap/drag-aware slot
//! index. Keeping the grouping algorithm here prevents GPU lowering and visual
//! connector code from growing separate HashMap/sort semantics.

use std::collections::BTreeMap;

use crate::app::PlacedGate;

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
