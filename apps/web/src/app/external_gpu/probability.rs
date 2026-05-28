use std::sync::Arc;

use crate::app::{CircuitColumnIndex, PlacedGate};
use crate::gates::GateKind;
use crate::gpu::{ExternalProbabilityUpload, ExternalProbabilityUploadBatch};
use crate::qubit_count::QubitCount;

pub(super) struct ExternalProbabilityRequest {
    pub(super) gate_id: u32,
    pub(super) column: CircuitColumnIndex,
    pub(super) span: usize,
    pub(super) base_bit: u32,
}

pub(super) fn collect_probability_requests(
    placed_gates: &[PlacedGate],
    qubits: QubitCount,
) -> Vec<ExternalProbabilityRequest> {
    let qubits = qubits.get();
    let Some(max_column) = placed_gates.iter().map(|gate| gate.column.as_usize()).max() else {
        return Vec::new();
    };
    let mut requests = Vec::new();
    for column in 0..=max_column {
        let mut displays: Vec<&PlacedGate> = placed_gates
            .iter()
            .filter(|gate| {
                gate.column.as_usize() == column
                    && gate.kind == GateKind::ProbabilityDisplay
                    && gate.wire < qubits
            })
            .collect();
        displays.sort_by(|a, b| a.id.cmp(&b.id));
        for display in displays {
            let span = display.span.clamp(1, 16).min(qubits - display.wire);
            let base_bit = (qubits - display.wire - span) as u32;
            requests.push(ExternalProbabilityRequest {
                gate_id: display.id,
                column: CircuitColumnIndex::new(column),
                span,
                base_bit,
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
                r#"{{"gate_id":{},"column":{},"span":{},"base_bit":{}}}"#,
                request.gate_id,
                request.column.as_usize(),
                request.span,
                request.base_bit
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
        uploads.push(ExternalProbabilityUpload {
            slot: slot_index as u32,
            probabilities: Arc::from(values.into_boxed_slice()),
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

    #[test]
    fn serializes_probability_output_request() {
        let requests = collect_probability_requests(
            &[PlacedGate::new(
                2,
                GateKind::ProbabilityDisplay,
                crate::app::CircuitColumnIndex::new(1),
                0,
                1,
                None,
            )],
            qubit_count(1),
        );

        assert_eq!(
            probability_requests_json(&requests),
            r#"[{"gate_id":2,"column":1,"span":1,"base_bit":0}]"#,
        );
    }

    #[test]
    fn serializes_probability_span_base_bit() {
        let requests = collect_probability_requests(
            &[PlacedGate::new(
                2,
                GateKind::ProbabilityDisplay,
                crate::app::CircuitColumnIndex::new(1),
                0,
                2,
                None,
            )],
            qubit_count(3),
        );

        assert_eq!(
            probability_requests_json(&requests),
            r#"[{"gate_id":2,"column":1,"span":2,"base_bit":1}]"#,
        );
    }

    #[test]
    fn external_probability_slot_matches_collection_order() {
        let requests = collect_probability_requests(
            &[
                PlacedGate::new(
                    4,
                    GateKind::ProbabilityDisplay,
                    crate::app::CircuitColumnIndex::new(1),
                    0,
                    1,
                    None,
                ),
                PlacedGate::new(
                    3,
                    GateKind::ProbabilityDisplay,
                    crate::app::CircuitColumnIndex::new(1),
                    1,
                    1,
                    None,
                ),
            ],
            qubit_count(2),
        );

        assert_eq!(probability_slot_to_gate_id(&requests), vec![3, 4]);
    }
}
