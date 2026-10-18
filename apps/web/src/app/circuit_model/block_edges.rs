//! Column boundaries a circuit block may start or end at.
//!
//! A block edge never cuts through a multi-column gate (Amplitude / Density
//! displays), and a block always wraps at least one column that holds a gate:
//! blocks over empty columns vanish at the next `compact_empty_steps`.

use super::{CircuitColumnIndex, PlacedGate};
use crate::layout::gate_width_cols;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BlockEdges {
    /// `allowed[b]` for every boundary `0..=gate_end`.
    allowed: Vec<bool>,
    /// `occupied[c]` for every column `0..gate_end`.
    occupied: Vec<bool>,
}

impl BlockEdges {
    pub(crate) fn for_gates(gates: &[PlacedGate]) -> Self {
        let spans: Vec<(usize, usize)> = gates
            .iter()
            .filter_map(|gate| {
                let start = gate.column.as_usize();
                let end = gate
                    .column
                    .checked_add(gate_width_cols(gate.kind, gate.span.get()))?;
                Some((start, end.as_usize()))
            })
            .collect();
        let gate_end = spans.iter().map(|&(_, end)| end).max().unwrap_or(0);
        let mut allowed = vec![true; gate_end + 1];
        let mut occupied = vec![false; gate_end];
        for &(start, end) in &spans {
            allowed[start + 1..end].fill(false);
            occupied[start..end].fill(true);
        }
        Self { allowed, occupied }
    }

    /// First boundary after the last gate column.
    pub(crate) fn gate_end(&self) -> usize {
        self.occupied.len()
    }

    pub(crate) fn allows(&self, boundary: usize) -> bool {
        self.allowed.get(boundary).copied().unwrap_or(false)
    }

    pub(crate) fn has_gate_in(&self, start: usize, end: usize) -> bool {
        self.occupied
            .get(start..end.min(self.occupied.len()))
            .is_some_and(|columns| columns.contains(&true))
    }

    /// The narrowest edge-aligned range around `column`, when that column
    /// holds a gate.
    pub(crate) fn range_around(
        &self,
        column: CircuitColumnIndex,
    ) -> Option<(CircuitColumnIndex, CircuitColumnIndex)> {
        let column = column.as_usize();
        if !self.occupied.get(column).copied().unwrap_or(false) {
            return None;
        }
        let start = (0..=column).rev().find(|&b| self.allows(b))?;
        let end = (column + 1..=self.gate_end()).find(|&b| self.allows(b))?;
        Some((CircuitColumnIndex::new(start), CircuitColumnIndex::new(end)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edges(json: &str) -> BlockEdges {
        BlockEdges::for_gates(&crate::url_circuit::parse_circuit_json(json).gates)
    }

    fn range(start: usize, end: usize) -> Option<(CircuitColumnIndex, CircuitColumnIndex)> {
        Some((CircuitColumnIndex::new(start), CircuitColumnIndex::new(end)))
    }

    #[test]
    fn range_around_a_single_column_gate_is_that_column() {
        assert_eq!(
            edges(r#"{"cols":[["H"],["X"]]}"#).range_around(CircuitColumnIndex::new(1)),
            range(1, 2)
        );
    }

    #[test]
    fn range_around_the_second_column_of_a_wide_gate_wraps_the_whole_gate() {
        assert_eq!(
            edges(r#"{"cols":[["H"],["Amps2"],[1],["X"]]}"#)
                .range_around(CircuitColumnIndex::new(2)),
            range(1, 3)
        );
    }

    #[test]
    fn range_around_an_empty_column_is_none() {
        assert_eq!(
            edges(r#"{"cols":[["H"],[1],["X"]]}"#).range_around(CircuitColumnIndex::new(1)),
            None
        );
    }

    #[test]
    fn range_around_the_trailing_area_is_none() {
        assert_eq!(
            edges(r#"{"cols":[["H"]]}"#).range_around(CircuitColumnIndex::new(1)),
            None
        );
    }

    #[test]
    fn edges_do_not_cut_through_a_wide_gate() {
        assert!(!edges(r#"{"cols":[["Amps2"],[1],["X"]]}"#).allows(1));
    }

    #[test]
    fn edges_stop_after_the_last_gate() {
        assert!(!edges(r#"{"cols":[["H"]]}"#).allows(2));
    }

    #[test]
    fn empty_columns_hold_no_gate() {
        assert!(!edges(r#"{"cols":[["H"],[1],["X"]]}"#).has_gate_in(1, 2));
    }
}
