use std::sync::Arc;

use crate::app::{CircuitColumnIndex, PlacedGate};
use crate::gates::GateKind;
use crate::gpu::{ExternalAmplitudeUpload, ExternalAmplitudeUploadBatch};

pub(super) struct ExternalAmplitudeRequest {
    pub(super) gate_id: u32,
    pub(super) column: CircuitColumnIndex,
    pub(super) span: usize,
    pub(super) base_bit: u32,
    pub(super) control_mask: u32,
    pub(super) control_value: u32,
    pub(super) phase_lock_enabled: bool,
}

pub(super) fn collect_amplitude_requests(
    placed_gates: &[PlacedGate],
    qubits: usize,
) -> Vec<ExternalAmplitudeRequest> {
    let Some(max_column) = placed_gates.iter().map(|gate| gate.column.as_usize()).max() else {
        return Vec::new();
    };
    let mut requests = Vec::new();
    for column in 0..=max_column {
        let column_gates: Vec<&PlacedGate> = placed_gates
            .iter()
            .filter(|gate| gate.column.as_usize() == column && gate.wire < qubits)
            .collect();
        let mut control_mask = 0u32;
        let mut control_value = 0u32;
        for gate in &column_gates {
            let bit = (qubits - 1 - gate.wire) as u32;
            if gate.kind == GateKind::Control {
                control_mask |= 1u32 << bit;
                control_value |= 1u32 << bit;
            } else if gate.kind == GateKind::AntiControl {
                control_mask |= 1u32 << bit;
            }
        }
        let mut displays: Vec<&PlacedGate> = column_gates
            .into_iter()
            .filter(|gate| gate.kind == GateKind::AmplitudeDisplay)
            .collect();
        displays.sort_by(|a, b| a.id.cmp(&b.id));
        for display in displays {
            let span = display.span.clamp(1, 16).min(qubits - display.wire);
            let base_bit = (qubits - display.wire - span) as u32;
            requests.push(ExternalAmplitudeRequest {
                gate_id: display.id,
                column: CircuitColumnIndex::new(column),
                span,
                base_bit,
                control_mask,
                control_value,
                phase_lock_enabled: span != qubits,
            });
        }
    }
    requests
}

pub(super) fn amplitude_requests_json(requests: &[ExternalAmplitudeRequest]) -> String {
    let entries: Vec<String> = requests
        .iter()
        .map(|request| {
            format!(
                concat!(
                    "{{\"gate_id\":{},\"column\":{},\"span\":{}",
                    ",\"base_bit\":{},\"control_mask\":{}",
                    ",\"control_value\":{},\"phase_lock_enabled\":{}}}"
                ),
                request.gate_id,
                request.column.as_usize(),
                request.span,
                request.base_bit,
                request.control_mask,
                request.control_value,
                request.phase_lock_enabled
            )
        })
        .collect();
    format!("[{}]", entries.join(","))
}

pub(super) fn amplitude_slot_to_gate_id(requests: &[ExternalAmplitudeRequest]) -> Vec<u32> {
    requests.iter().map(|request| request.gate_id).collect()
}

pub(super) fn parse_amplitude_upload_batch(
    message: &str,
    generation: u64,
    slot_to_gate_id: &[u32],
) -> Option<ExternalAmplitudeUploadBatch> {
    if slot_to_gate_id.is_empty() {
        return None;
    }
    parse_amplitude_upload_batch_impl(message, generation, slot_to_gate_id)
}

#[cfg(target_arch = "wasm32")]
fn parse_amplitude_upload_batch_impl(
    message: &str,
    generation: u64,
    slot_to_gate_id: &[u32],
) -> Option<ExternalAmplitudeUploadBatch> {
    use wasm_bindgen::JsCast;

    let root = js_sys::JSON::parse(message).ok()?;
    let amplitudes = js_sys::Reflect::get(&root, &wasm_bindgen::JsValue::from_str("amplitudes"))
        .ok()?
        .dyn_into::<js_sys::Array>()
        .ok()?;
    if slot_to_gate_id.len() > crate::gpu::MAX_AMPLITUDE_SLOTS {
        return None;
    }
    let mut uploads = Vec::new();
    let mut seen_slots = vec![false; slot_to_gate_id.len()];
    for value in amplitudes.iter() {
        let gate_id = js_u32_prop(&value, "gate_id")?;
        let slot_index = slot_to_gate_id.iter().position(|id| *id == gate_id)?;
        if slot_index >= crate::gpu::MAX_AMPLITUDE_SLOTS || seen_slots[slot_index] {
            return None;
        }
        seen_slots[slot_index] = true;
        let slot = slot_index as u32;
        let span = js_u32_prop(&value, "span")?;
        if span == 0 || span > 16 {
            return None;
        }
        let outcomes = 1usize << span;
        let ket = js_array_prop(&value, "ket")?;
        let incoherent = js_array_prop(&value, "incoherent")?;
        if ket.length() < outcomes as u32 || incoherent.length() < outcomes as u32 {
            return None;
        }
        let mut coherent = Vec::with_capacity(outcomes * 2);
        for index in 0..outcomes as u32 {
            let pair = ket.get(index).dyn_into::<js_sys::Array>().ok()?;
            coherent.push(pair.get(0).as_f64()? as f32);
            coherent.push(pair.get(1).as_f64()? as f32);
        }
        let mut incoherent_values = Vec::with_capacity(outcomes);
        for index in 0..outcomes as u32 {
            incoherent_values.push(incoherent.get(index).as_f64()? as f32);
        }
        let quality = js_f32_prop(&value, "quality")?;
        let phase_lock_index = js_f32_prop(&value, "phase_lock_index")?;
        uploads.push(ExternalAmplitudeUpload {
            slot,
            coherent: Arc::from(coherent.into_boxed_slice()),
            incoherent: Arc::from(incoherent_values.into_boxed_slice()),
            meta: [quality, phase_lock_index, span as f32, 0.0],
        });
    }
    if uploads.len() != slot_to_gate_id.len() || seen_slots.iter().any(|seen| !*seen) {
        return None;
    }
    Some(ExternalAmplitudeUploadBatch {
        generation,
        slot_to_gate_id: Arc::from(slot_to_gate_id.to_vec().into_boxed_slice()),
        uploads: Arc::from(uploads.into_boxed_slice()),
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_amplitude_upload_batch_impl(
    _message: &str,
    _generation: u64,
    _slot_to_gate_id: &[u32],
) -> Option<ExternalAmplitudeUploadBatch> {
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

#[cfg(target_arch = "wasm32")]
fn js_f32_prop(value: &wasm_bindgen::JsValue, name: &str) -> Option<f32> {
    js_sys::Reflect::get(value, &wasm_bindgen::JsValue::from_str(name))
        .ok()?
        .as_f64()
        .map(|raw| raw as f32)
}

#[cfg(test)]
mod tests {
    use super::{amplitude_requests_json, amplitude_slot_to_gate_id, collect_amplitude_requests};
    use crate::app::PlacedGate;
    use crate::gates::GateKind;

    #[test]
    fn serializes_amplitude_output_request() {
        let requests = collect_amplitude_requests(
            &[PlacedGate::new(
                2,
                GateKind::AmplitudeDisplay,
                crate::app::CircuitColumnIndex::new(1),
                0,
                1,
                None,
            )],
            1,
        );
        assert_eq!(
            amplitude_requests_json(&requests),
            r#"[{"gate_id":2,"column":1,"span":1,"base_bit":0,"control_mask":0,"control_value":0,"phase_lock_enabled":false}]"#,
        );
    }

    #[test]
    fn serializes_column_control_for_amplitude_output_request() {
        let requests = collect_amplitude_requests(
            &[
                PlacedGate::new(
                    1,
                    GateKind::Control,
                    crate::app::CircuitColumnIndex::new(0),
                    0,
                    1,
                    None,
                ),
                PlacedGate::new(
                    2,
                    GateKind::AmplitudeDisplay,
                    crate::app::CircuitColumnIndex::new(0),
                    1,
                    1,
                    None,
                ),
            ],
            2,
        );
        assert_eq!(requests[0].control_mask, 2);
    }

    #[test]
    fn serializes_column_anti_control_for_amplitude_output_request() {
        let requests = collect_amplitude_requests(
            &[
                PlacedGate::new(
                    1,
                    GateKind::AntiControl,
                    crate::app::CircuitColumnIndex::new(0),
                    0,
                    1,
                    None,
                ),
                PlacedGate::new(
                    2,
                    GateKind::AmplitudeDisplay,
                    crate::app::CircuitColumnIndex::new(0),
                    1,
                    1,
                    None,
                ),
            ],
            2,
        );
        assert_eq!(
            (requests[0].control_mask, requests[0].control_value),
            (2, 0)
        );
    }

    #[test]
    fn external_amplitude_slot_matches_collection_order() {
        let requests = collect_amplitude_requests(
            &[
                PlacedGate::new(
                    4,
                    GateKind::AmplitudeDisplay,
                    crate::app::CircuitColumnIndex::new(1),
                    0,
                    1,
                    None,
                ),
                PlacedGate::new(
                    3,
                    GateKind::AmplitudeDisplay,
                    crate::app::CircuitColumnIndex::new(1),
                    1,
                    1,
                    None,
                ),
            ],
            2,
        );
        assert_eq!(amplitude_slot_to_gate_id(&requests), vec![3, 4]);
    }
}
