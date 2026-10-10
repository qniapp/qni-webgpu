//! Which measurement each conditional gate (`X<name`) reads.
//!
//! qni's `simulator.ts::runStep` keeps one `flags` map per run: every
//! `Measure>name` overwrites `flags[name]` and every `X<name` reads the
//! current value, treating a never-written flag as false. qni-webgpu runs a
//! column's unitaries before its measurements (see `linearize_ops`), so a
//! conditional gate reads the last `Measure>name` in an *earlier* column.
//! Within one column, the bottom-most `Measure>name` wins, matching the wire
//! order in which qni serialises a step's measurement targets.
//!
//! Only gate structure is resolved here. The measured values themselves stay
//! on the GPU: the state compute shader and the gate body overlay read them
//! from the measurement aux buffer.

use std::collections::HashMap;

use super::SimulationColumnAnalysis;
use crate::app::GateId;
use crate::gates::{FlagName, GateFlag};

/// Conditional gate → the measurement gate whose outcome it reads.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct FlagSources {
    sources: HashMap<GateId, FlagSource>,
}

/// What a conditional gate reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FlagSource {
    /// The outcome sampled by this measurement gate.
    Measurement(GateId),
    /// No earlier measurement writes the flag, so it is false and the gate
    /// never applies.
    NeverSet,
}

impl FlagSources {
    pub(crate) fn from_columns(analysis: &SimulationColumnAnalysis<'_>) -> Self {
        let mut latest: HashMap<&FlagName, GateId> = HashMap::new();
        let mut sources = HashMap::new();
        for column in analysis.columns() {
            for gate in column.gates() {
                if let Some(GateFlag::If(name)) = &gate.flag {
                    let source = latest
                        .get(name)
                        .map_or(FlagSource::NeverSet, |id| FlagSource::Measurement(*id));
                    sources.insert(gate.id, source);
                }
            }
            let mut measurements: Vec<_> = column
                .gates()
                .iter()
                .filter_map(|gate| match &gate.flag {
                    Some(GateFlag::Set(name)) => Some((gate.wire, name, gate.id)),
                    _ => None,
                })
                .collect();
            measurements.sort_by_key(|(wire, _, _)| *wire);
            for (_, name, id) in measurements {
                latest.insert(name, id);
            }
        }
        Self { sources }
    }

    /// `None` when `gate_id` is not a conditional gate of the analysed plan.
    pub(crate) fn source(&self, gate_id: GateId) -> Option<FlagSource> {
        self.sources.get(&gate_id).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{CircuitColumnIndex, PlacedGate, WireIndex};
    use crate::gates::{GateKind, GateSpan};
    use crate::qubit_count::QubitCount;

    fn gate(id: u32, kind: GateKind, column: usize, wire: usize, flag: &str) -> PlacedGate {
        let gate = PlacedGate::new(
            GateId::from_u32(id),
            kind,
            CircuitColumnIndex::new(column),
            WireIndex::new(wire),
            GateSpan::SINGLE,
            None,
        );
        let Some(name) = FlagName::parse(flag) else {
            return gate;
        };
        let flag = if kind == GateKind::Measurement {
            GateFlag::Set(name)
        } else {
            GateFlag::If(name)
        };
        gate.with_flag(flag).expect("test flag fits its gate")
    }

    fn sources(gates: &[PlacedGate]) -> FlagSources {
        let qubits = QubitCount::try_new(3).expect("test qubit count is non-zero");
        FlagSources::from_columns(&SimulationColumnAnalysis::from_gates(gates, qubits))
    }

    #[test]
    fn conditional_gate_reads_an_earlier_measurement() {
        let gates = [
            gate(1, GateKind::Measurement, 0, 0, "a"),
            gate(2, GateKind::X, 1, 1, "a"),
        ];

        assert_eq!(
            sources(&gates).source(GateId::from_u32(2)),
            Some(FlagSource::Measurement(GateId::from_u32(1)))
        );
    }

    #[test]
    fn unset_flag_never_applies() {
        let gates = [gate(1, GateKind::X, 0, 0, "a")];

        assert_eq!(
            sources(&gates).source(GateId::from_u32(1)),
            Some(FlagSource::NeverSet)
        );
    }

    #[test]
    fn same_column_measurement_does_not_feed_the_condition() {
        let gates = [
            gate(1, GateKind::Measurement, 0, 0, "a"),
            gate(2, GateKind::X, 0, 1, "a"),
        ];

        assert_eq!(
            sources(&gates).source(GateId::from_u32(2)),
            Some(FlagSource::NeverSet)
        );
    }

    #[test]
    fn later_measurement_overwrites_the_flag() {
        let gates = [
            gate(1, GateKind::Measurement, 0, 0, "a"),
            gate(2, GateKind::Measurement, 1, 1, "a"),
            gate(3, GateKind::X, 2, 2, "a"),
        ];

        assert_eq!(
            sources(&gates).source(GateId::from_u32(3)),
            Some(FlagSource::Measurement(GateId::from_u32(2)))
        );
    }

    #[test]
    fn bottom_measurement_wins_within_a_column() {
        let gates = [
            gate(5, GateKind::Measurement, 0, 1, "a"),
            gate(6, GateKind::Measurement, 0, 0, "a"),
            gate(7, GateKind::X, 1, 2, "a"),
        ];

        assert_eq!(
            sources(&gates).source(GateId::from_u32(7)),
            Some(FlagSource::Measurement(GateId::from_u32(5)))
        );
    }

    #[test]
    fn flags_are_independent_by_name() {
        let gates = [
            gate(1, GateKind::Measurement, 0, 0, "a"),
            gate(2, GateKind::X, 1, 1, "b"),
        ];

        assert_eq!(
            sources(&gates).source(GateId::from_u32(2)),
            Some(FlagSource::NeverSet)
        );
    }

    #[test]
    fn unconditional_gate_has_no_source() {
        let gates = [gate(1, GateKind::X, 0, 0, "")];

        assert_eq!(sources(&gates).source(GateId::from_u32(1)), None);
    }
}
