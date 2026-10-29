use std::collections::BTreeMap;

use crate::app::PlacedGate;
use crate::gates::{ColumnControls, GateFlag, GateKind, ParametricAngle};
use crate::qubit_count::QubitCount;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct PhaseGroupKey<'a> {
    angle: ParametricAngle,
    flag: Option<&'a GateFlag>,
}

impl<'a> PhaseGroupKey<'a> {
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
                        GateKind::Control | GateKind::ZDetectControlReset => {
                            controls.add_control(bit);
                        }
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

/// 伸縮できる範囲の外にあるパリティ制御を、上のワイヤから順に返す。先頭が集約先である。
pub(crate) fn external_parity_controls<'a>(
    gates: &[&'a PlacedGate],
    qubits: QubitCount,
) -> Vec<&'a PlacedGate> {
    let mut parity_controls: Vec<&PlacedGate> = gates
        .iter()
        .copied()
        .filter(|gate| {
            matches!(
                gate.kind,
                GateKind::ZParityControl | GateKind::XParityControl
            )
        })
        .filter(|parity| !resizable_span_covers(gates, parity.wire.as_usize(), qubits))
        .collect();
    parity_controls.sort_by_key(|gate| gate.wire);
    parity_controls
}

fn resizable_span_covers(gates: &[&PlacedGate], wire: usize, qubits: QubitCount) -> bool {
    gates.iter().any(|gate| {
        gate.kind.is_resizable_span()
            && gate.wire.as_usize() <= wire
            && wire
                < gate.wire.as_usize()
                    + gate
                        .span
                        .clamped_for(gate.kind, qubits.get() - gate.wire.as_usize())
                        .get()
    })
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

#[cfg(test)]
mod tests {
    use super::external_parity_controls;
    use crate::app::{CircuitColumnIndex, GateId, PlacedGate, WireIndex};
    use crate::gates::{GateKind, GateSpan};
    use crate::qubit_count::QubitCount;

    fn placed(kind: GateKind, wire: usize, span: usize) -> PlacedGate {
        PlacedGate::new(
            GateId::from_u32(wire as u32 + 1),
            kind,
            CircuitColumnIndex::new(0),
            WireIndex::new(wire),
            GateSpan::try_new(span).expect("span is at least 1"),
            None,
        )
    }

    fn parity_wires(gates: &[PlacedGate]) -> Vec<usize> {
        let refs: Vec<&PlacedGate> = gates.iter().collect();
        external_parity_controls(&refs, QubitCount::try_new(8).expect("8 qubits"))
            .into_iter()
            .map(|gate| gate.wire.as_usize())
            .collect()
    }

    #[test]
    fn external_parity_controls_list_the_top_wire_first() {
        let gates = [
            placed(GateKind::ZParityControl, 3, 1),
            placed(GateKind::AntiControl, 4, 1),
            placed(GateKind::Control, 2, 1),
            placed(GateKind::XParityControl, 1, 1),
        ];
        assert_eq!(parity_wires(&gates), vec![1, 3]);
    }

    #[test]
    fn external_parity_controls_keep_either_basis() {
        let wires = [GateKind::ZParityControl, GateKind::XParityControl]
            .map(|kind| parity_wires(&[placed(kind, 1, 1)]));
        assert_eq!(wires, [vec![1], vec![1]]);
    }

    #[test]
    fn external_parity_controls_omit_wires_inside_a_resizable_span() {
        let gates = [
            placed(GateKind::ZParityControl, 2, 1),
            placed(GateKind::XParityControl, 3, 1),
            placed(GateKind::QftGate, 1, 2),
        ];
        assert_eq!(parity_wires(&gates), vec![3]);
    }

    #[test]
    fn external_parity_controls_are_empty_when_every_parity_is_covered() {
        let gates = [
            placed(GateKind::ZParityControl, 2, 1),
            placed(GateKind::XParityControl, 7, 1),
            placed(GateKind::QftGate, 1, 32),
        ];
        assert_eq!(parity_wires(&gates), Vec::<usize>::new());
    }
}
