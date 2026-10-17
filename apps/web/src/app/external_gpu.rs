mod amplitude;
mod app_adapter;
mod bloch;
mod density;
mod probability;
mod session;
mod test_hooks;
mod transport;

pub(crate) use session::{ExternalGpuSession, Invalidation};

#[cfg(all(test, not(target_arch = "wasm32")))]
mod parser_fixtures;

pub(crate) use qni_web_external_gpu_model::{
    format_gpu_duration, qiskit_run_payload_with_display_outputs, ExternalGpuStatus, GpuFailure,
    Shots,
};
#[cfg(target_arch = "wasm32")]
pub(crate) use qni_web_external_gpu_model::{short_failure_label, unsupported_gate_from_message};

use super::{ExecMode, QniApp};

/// Controls at the display-save point, while parity is still gathered.
/// Keep anchor filtering in sync with `linearize_ops`.
fn display_controls(
    gates: &[&crate::app::PlacedGate],
    mut controls: crate::gates::ColumnControls,
    qubits: crate::qubit_count::QubitCount,
) -> crate::gates::ColumnControls {
    use crate::gates::GateKind;

    let anchor = gates
        .iter()
        .filter(|gate| {
            matches!(
                gate.kind,
                GateKind::ZParityControl | GateKind::XParityControl
            )
        })
        .filter(|parity| {
            !gates.iter().any(|gate| {
                gate.kind.is_resizable_span()
                    && gate.wire.as_usize() <= parity.wire.as_usize()
                    && parity.wire.as_usize()
                        < gate.wire.as_usize()
                            + gate
                                .span
                                .clamped_for(gate.kind, qubits.get() - gate.wire.as_usize())
                                .get()
            })
        })
        .min_by_key(|gate| gate.wire);
    if let Some(anchor) = anchor {
        controls.add_control(
            anchor
                .wire
                .to_qubit_bit(qubits)
                .expect("simulation column wires are within the register"),
        );
    }
    controls
}

#[cfg(any(target_arch = "wasm32", test))]
fn display_defined_marker(defined: Option<bool>) -> f32 {
    if defined == Some(false) {
        -1.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod defined_tests {
    #[test]
    fn external_gpu_defined_marker_preserves_undefined_state() {
        assert_eq!(
            [Some(false), Some(true), None].map(super::display_defined_marker),
            [-1.0, 0.0, 0.0]
        );
    }
}

#[cfg(test)]
mod parity_display_tests {
    use crate::app::{CircuitColumnIndex, GateId, PlacedGate, WireIndex};
    use crate::gates::{GateKind, GateSpan};
    use crate::qubit_count::QubitCount;
    use crate::simulation_plan::SimulationColumnAnalysis;

    fn gate(kind: GateKind, wire: usize) -> PlacedGate {
        PlacedGate::new(
            GateId::from_u32(wire as u32 + 1),
            kind,
            CircuitColumnIndex::new(0),
            WireIndex::new(wire),
            GateSpan::SINGLE,
            None,
        )
    }

    fn display_payloads(extra: &[PlacedGate]) -> Vec<String> {
        let qubits = QubitCount::try_new(8).unwrap();
        [
            GateKind::BlochDisplay,
            GateKind::ProbabilityDisplay,
            GateKind::AmplitudeDisplay,
            GateKind::DensityMatrixDisplay,
        ]
        .map(|kind| {
            let mut gates = vec![gate(kind, 0)];
            gates.extend_from_slice(extra);
            let analysis = SimulationColumnAnalysis::from_gates(&gates, qubits);
            match kind {
                GateKind::BlochDisplay => super::bloch::bloch_requests_json(
                    &super::bloch::collect_bloch_requests(&analysis, qubits),
                ),
                GateKind::ProbabilityDisplay => super::probability::probability_requests_json(
                    &super::probability::collect_probability_requests(&analysis, qubits),
                ),
                GateKind::AmplitudeDisplay => super::amplitude::amplitude_requests_json(
                    &super::amplitude::collect_amplitude_requests(&analysis, qubits),
                ),
                GateKind::DensityMatrixDisplay => super::density::density_requests_json(
                    &super::density::collect_density_requests(&analysis, qubits),
                ),
                _ => unreachable!(),
            }
        })
        .to_vec()
    }

    fn expected_payloads(mask: u32, value: u32) -> Vec<String> {
        let controls = format!("\"control_mask\":{mask},\"control_value\":{value}");
        let span = format!("[{{\"gate_id\":1,\"column\":0,\"span\":1,\"base_bit\":0,{controls}");
        vec![
            format!("[{{\"gate_id\":1,\"column\":0,\"wire\":0,{controls}}}]"),
            format!("{span}}}]"),
            format!("{span},\"phase_lock_enabled\":true}}]"),
            format!("{span}}}]"),
        ]
    }

    #[test]
    fn all_display_payloads_add_only_topmost_parity_anchor() {
        // Deliberately unsorted: xpar at wire 1 is the anchor, not zpar at 3.
        let gates = [
            gate(GateKind::ZParityControl, 3),
            gate(GateKind::AntiControl, 4),
            gate(GateKind::Control, 2),
            gate(GateKind::XParityControl, 1),
        ];
        assert_eq!(display_payloads(&gates), expected_payloads(22, 6));
    }

    #[test]
    fn all_display_payloads_accept_either_parity_basis() {
        let actual = [GateKind::ZParityControl, GateKind::XParityControl]
            .map(|kind| display_payloads(&[gate(kind, 1)]));
        assert_eq!(actual, [expected_payloads(2, 2), expected_payloads(2, 2)]);
    }

    #[test]
    fn all_display_payloads_skip_parity_inside_resizable_span() {
        let mut span = gate(GateKind::QftGate, 1);
        span.span = GateSpan::try_new(2).unwrap();
        let gates = [
            gate(GateKind::ZParityControl, 2),
            gate(GateKind::XParityControl, 3),
            span,
        ];
        assert_eq!(display_payloads(&gates), expected_payloads(8, 8));
    }

    #[test]
    fn all_display_payloads_drop_parity_when_no_anchor_remains() {
        let mut span = gate(GateKind::QftGate, 1);
        span.span = GateSpan::try_new(32).unwrap();
        let gates = [
            gate(GateKind::ZParityControl, 2),
            gate(GateKind::XParityControl, 7),
            span,
        ];
        assert_eq!(display_payloads(&gates), expected_payloads(0, 0));
    }

    #[test]
    fn resizable_displays_drop_parity_on_covered_wire() {
        let qubits = QubitCount::try_new(8).unwrap();
        let actual = [
            GateKind::ProbabilityDisplay,
            GateKind::AmplitudeDisplay,
            GateKind::DensityMatrixDisplay,
        ]
        .map(|kind| {
            let mut display = gate(kind, 0);
            display.span = GateSpan::try_new(2).unwrap();
            let gates = [display, gate(GateKind::ZParityControl, 1)];
            let analysis = SimulationColumnAnalysis::from_gates(&gates, qubits);
            let controls = match kind {
                GateKind::ProbabilityDisplay => {
                    super::probability::collect_probability_requests(&analysis, qubits)[0].controls
                }
                GateKind::AmplitudeDisplay => {
                    super::amplitude::collect_amplitude_requests(&analysis, qubits)[0].controls
                }
                GateKind::DensityMatrixDisplay => {
                    super::density::collect_density_requests(&analysis, qubits)[0].controls
                }
                _ => unreachable!(),
            };
            (controls.mask(), controls.value())
        });
        assert_eq!(actual, [(0, 0); 3]);
    }
}
