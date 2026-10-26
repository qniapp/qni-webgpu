#[cfg(target_arch = "wasm32")]
use std::sync::Arc;

use crate::app::CircuitColumnIndex;
use crate::gates::{ColumnControls, GateKind};
use crate::gpu::ExternalProbabilityUploadBatch;
#[cfg(target_arch = "wasm32")]
use crate::gpu::{ExternalProbabilityUpload, ProbabilityDistribution};
use crate::qubit_bit::QubitBit;
use crate::qubit_count::QubitCount;
use crate::simulation_plan::SimulationColumnAnalysis;

pub(super) struct ExternalProbabilityRequest {
    pub(super) gate_id: u32,
    pub(super) column: CircuitColumnIndex,
    pub(super) span: usize,
    pub(super) base_bit: QubitBit,
    pub(super) controls: ColumnControls,
}

pub(super) fn collect_probability_requests(
    analysis: &SimulationColumnAnalysis<'_>,
    qubits: QubitCount,
) -> Vec<ExternalProbabilityRequest> {
    let n = qubits.get();
    let mut requests = Vec::new();
    for column in analysis.columns() {
        // Both displays consume the same conditional marginals; Sample draws in the GPU shader.
        for display in column
            .displays(GateKind::ProbabilityDisplay)
            .into_iter()
            .chain(column.displays(GateKind::SampleDisplay))
        {
            let span = display
                .span
                .clamped_for(display.kind, n - display.wire.as_usize());
            let base_bit = display
                .wire
                .to_qubit_bit(qubits)
                .expect("display wire is within the register");
            requests.push(ExternalProbabilityRequest {
                gate_id: display.id.as_u32(),
                column: CircuitColumnIndex::new(column.slot),
                span: span.get(),
                base_bit,
                controls: super::display_controls(column.gates(), column.controls(), qubits),
            });
        }
    }
    requests
}

pub(super) fn probability_requests_json(requests: &[ExternalProbabilityRequest]) -> String {
    let entries: Vec<String> = requests
        .iter()
        .map(|request| {
            format!(
                concat!(
                    "{{\"gate_id\":{},\"column\":{},\"span\":{}",
                    ",\"base_bit\":{},\"control_mask\":{}",
                    ",\"control_value\":{}}}"
                ),
                request.gate_id,
                request.column.as_usize(),
                request.span,
                request.base_bit.as_u32(),
                request.controls.mask(),
                request.controls.value()
            )
        })
        .collect();
    format!("[{}]", entries.join(","))
}

pub(super) fn probability_slot_to_gate_id(requests: &[ExternalProbabilityRequest]) -> Vec<u32> {
    requests.iter().map(|request| request.gate_id).collect()
}

pub(super) fn parse_probability_upload_batch(
    message: &str,
    generation: u64,
    slot_to_gate_id: &[u32],
) -> Option<ExternalProbabilityUploadBatch> {
    if slot_to_gate_id.is_empty() {
        return None;
    }
    parse_probability_upload_batch_impl(message, generation, slot_to_gate_id)
}

#[cfg(target_arch = "wasm32")]
fn parse_probability_upload_batch_impl(
    message: &str,
    generation: u64,
    slot_to_gate_id: &[u32],
) -> Option<ExternalProbabilityUploadBatch> {
    use wasm_bindgen::JsCast;

    if slot_to_gate_id.len() > crate::gpu::MAX_PROBABILITY_SLOTS {
        return None;
    }
    let root = js_sys::JSON::parse(message).ok()?;
    let probability = js_sys::Reflect::get(&root, &wasm_bindgen::JsValue::from_str("probability"))
        .ok()?
        .dyn_into::<js_sys::Array>()
        .ok()?;
    let mut uploads = Vec::new();
    let mut seen_slots = vec![false; slot_to_gate_id.len()];
    for value in probability.iter() {
        let gate_id = js_u32_prop(&value, "gate_id")?;
        let slot_index = slot_to_gate_id.iter().position(|id| *id == gate_id)?;
        if slot_index >= crate::gpu::MAX_PROBABILITY_SLOTS || seen_slots[slot_index] {
            return None;
        }
        seen_slots[slot_index] = true;
        let span = js_u32_prop(&value, "span")?;
        if span == 0 || span > 16 {
            return None;
        }
        let outcomes = 1usize << span;
        let probabilities = js_array_prop(&value, "probabilities")?;
        if probabilities.length() < outcomes as u32 {
            return None;
        }
        let mut values = Vec::with_capacity(outcomes);
        for index in 0..outcomes as u32 {
            values.push(probabilities.get(index).as_f64()? as f32);
        }
        let distribution =
            ProbabilityDistribution::try_new(Arc::from(values.into_boxed_slice())).ok()?;
        uploads.push(ExternalProbabilityUpload {
            slot: slot_index as u32,
            probabilities: distribution,
        });
    }
    if uploads.len() != slot_to_gate_id.len() || seen_slots.iter().any(|seen| !*seen) {
        return None;
    }
    Some(ExternalProbabilityUploadBatch {
        generation,
        slot_to_gate_id: Arc::from(slot_to_gate_id.to_vec().into_boxed_slice()),
        uploads: Arc::from(uploads.into_boxed_slice()),
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_probability_upload_batch_impl(
    _message: &str,
    _generation: u64,
    _slot_to_gate_id: &[u32],
) -> Option<ExternalProbabilityUploadBatch> {
    #[cfg(test)]
    return super::parser_fixtures::take_probability(_message, _generation, _slot_to_gate_id);
    #[cfg(not(test))]
    None
}

#[cfg(target_arch = "wasm32")]
fn js_array_prop(value: &wasm_bindgen::JsValue, name: &str) -> Option<js_sys::Array> {
    use wasm_bindgen::JsCast;

    js_sys::Reflect::get(value, &wasm_bindgen::JsValue::from_str(name))
        .ok()?
        .dyn_into::<js_sys::Array>()
        .ok()
}

#[cfg(target_arch = "wasm32")]
fn js_u32_prop(value: &wasm_bindgen::JsValue, name: &str) -> Option<u32> {
    let raw = js_sys::Reflect::get(value, &wasm_bindgen::JsValue::from_str(name))
        .ok()?
        .as_f64()?;
    if raw < 0.0 || raw > u32::MAX as f64 {
        return None;
    }
    Some(raw as u32)
}

#[cfg(test)]
mod tests {
    use super::{
        collect_probability_requests, probability_requests_json, probability_slot_to_gate_id,
    };
    use crate::app::PlacedGate;
    use crate::gates::GateKind;
    use crate::qubit_count::QubitCount;

    fn qubit_count(value: usize) -> QubitCount {
        QubitCount::try_new(value).expect("test qubit count must be non-zero")
    }

    fn requests(
        gates: &[PlacedGate],
        qubits: QubitCount,
    ) -> Vec<super::ExternalProbabilityRequest> {
        let columns = crate::simulation_plan::SimulationColumnAnalysis::from_gates(gates, qubits);
        collect_probability_requests(&columns, qubits)
    }

    #[test]
    fn sample_requests_preserve_controls_and_drop_covered_parity() {
        let actual = [
            (r#"{"cols":[["•","Sample1"]]}"#, 1, 1),
            (r#"{"cols":[["◦","Sample1"]]}"#, 1, 0),
            (r#"{"cols":[["zpar","zpar","Sample1"]]}"#, 1, 1),
            (r#"{"cols":[["xpar","xpar","Sample1"]]}"#, 1, 1),
            (r#"{"cols":[["Sample3","zpar","xpar"]]}"#, 0, 0),
        ]
        .map(|(json, mask, value)| {
            let gates = crate::url_circuit::parse_circuit_json(json).gates;
            let request = requests(&gates, qubit_count(3));
            (
                (request[0].controls.mask(), request[0].controls.value()),
                (mask, value),
            )
        });
        assert_eq!(
            actual.map(|(actual, _)| actual),
            actual.map(|(_, expected)| expected)
        );
    }

    #[test]
    fn serializes_sample_spans_clamped_at_register_bottom() {
        let actual = (1..=3)
            .map(|span| {
                let gate = PlacedGate::new(
                    crate::app::GateId::from_u32(7),
                    GateKind::SampleDisplay,
                    crate::app::CircuitColumnIndex::new(2),
                    crate::app::WireIndex::new(1),
                    crate::gates::GateSpan::try_new(span).unwrap(),
                    None,
                );
                probability_requests_json(&requests(&[gate], qubit_count(3)))
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, [1, 2, 2].map(|span| format!(
            "[{{\"gate_id\":7,\"column\":2,\"span\":{span},\"base_bit\":1,\"control_mask\":0,\"control_value\":0}}]"
        )));
    }

    #[test]
    fn serializes_sample8_request_from_url() {
        let gates = crate::url_circuit::parse_circuit_json(r#"{"cols":[["Sample8"]]}"#).gates;
        assert_eq!(
            probability_requests_json(&requests(&gates, qubit_count(8))),
            r#"[{"gate_id":1,"column":0,"span":8,"base_bit":0,"control_mask":0,"control_value":0}]"#
        );
    }

    #[test]
    fn serializes_probability_output_request() {
        let requests = requests(
            &[PlacedGate::new(
                crate::app::GateId::from_u32(2),
                GateKind::ProbabilityDisplay,
                crate::app::CircuitColumnIndex::new(1),
                crate::app::WireIndex::new(0),
                crate::gates::GateSpan::SINGLE,
                None,
            )],
            qubit_count(1),
        );

        assert_eq!(
            probability_requests_json(&requests),
            r#"[{"gate_id":2,"column":1,"span":1,"base_bit":0,"control_mask":0,"control_value":0}]"#,
        );
    }

    #[test]
    fn serializes_probability_span_base_bit() {
        let requests = requests(
            &[PlacedGate::new(
                crate::app::GateId::from_u32(2),
                GateKind::ProbabilityDisplay,
                crate::app::CircuitColumnIndex::new(1),
                crate::app::WireIndex::new(0),
                crate::gates::GateSpan::try_new(2).unwrap(),
                None,
            )],
            qubit_count(3),
        );

        assert_eq!(
            probability_requests_json(&requests),
            r#"[{"gate_id":2,"column":1,"span":2,"base_bit":0,"control_mask":0,"control_value":0}]"#,
        );
    }

    #[test]
    fn serializes_probability_controls() {
        let requests = requests(
            &[
                PlacedGate::new(
                    crate::app::GateId::from_u32(1),
                    GateKind::Control,
                    crate::app::CircuitColumnIndex::new(2),
                    crate::app::WireIndex::new(0),
                    crate::gates::GateSpan::SINGLE,
                    None,
                ),
                PlacedGate::new(
                    crate::app::GateId::from_u32(2),
                    GateKind::AntiControl,
                    crate::app::CircuitColumnIndex::new(2),
                    crate::app::WireIndex::new(1),
                    crate::gates::GateSpan::SINGLE,
                    None,
                ),
                PlacedGate::new(
                    crate::app::GateId::from_u32(3),
                    GateKind::ProbabilityDisplay,
                    crate::app::CircuitColumnIndex::new(2),
                    crate::app::WireIndex::new(2),
                    crate::gates::GateSpan::SINGLE,
                    None,
                ),
            ],
            qubit_count(3),
        );

        assert_eq!(
            probability_requests_json(&requests),
            r#"[{"gate_id":3,"column":2,"span":1,"base_bit":2,"control_mask":3,"control_value":1}]"#,
        );
    }

    #[test]
    fn serializes_detect_control_reset_probability_controls() {
        let gates = [GateKind::ZDetectControlReset, GateKind::ProbabilityDisplay]
            .into_iter()
            .enumerate()
            .map(|(wire, kind)| {
                PlacedGate::new(
                    crate::app::GateId::from_u32(wire as u32 + 1),
                    kind,
                    crate::app::CircuitColumnIndex::new(0),
                    crate::app::WireIndex::new(wire),
                    crate::gates::GateSpan::SINGLE,
                    None,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            probability_requests_json(&requests(&gates, qubit_count(2))),
            r#"[{"gate_id":2,"column":0,"span":1,"base_bit":1,"control_mask":1,"control_value":1}]"#,
        );
    }

    #[test]
    fn external_probability_slot_matches_collection_order() {
        let requests = requests(
            &[
                PlacedGate::new(
                    crate::app::GateId::from_u32(4),
                    GateKind::ProbabilityDisplay,
                    crate::app::CircuitColumnIndex::new(1),
                    crate::app::WireIndex::new(0),
                    crate::gates::GateSpan::SINGLE,
                    None,
                ),
                PlacedGate::new(
                    crate::app::GateId::from_u32(3),
                    GateKind::ProbabilityDisplay,
                    crate::app::CircuitColumnIndex::new(1),
                    crate::app::WireIndex::new(1),
                    crate::gates::GateSpan::SINGLE,
                    None,
                ),
            ],
            qubit_count(2),
        );

        assert_eq!(probability_slot_to_gate_id(&requests), vec![3, 4]);
    }
}
