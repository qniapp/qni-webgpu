use std::sync::Arc;

use crate::app::PlacedGate;
use crate::gates::GateKind;
use crate::gpu::{ExternalDensityUpload, ExternalDensityUploadBatch};
use crate::qubit_count::QubitCount;

pub(super) struct ExternalDensityRequest {
    pub(super) gate_id: u32,
    pub(super) column: usize,
    pub(super) span: usize,
    pub(super) base_bit: u32,
    pub(super) control_mask: u32,
    pub(super) control_value: u32,
}

pub(super) fn collect_density_requests(
    placed_gates: &[PlacedGate],
    qubits: QubitCount,
) -> Vec<ExternalDensityRequest> {
    let qubits = qubits.get();
    let Some(max_column) = placed_gates.iter().map(|gate| gate.column).max() else {
        return Vec::new();
    };
    let mut requests = Vec::new();
    for column in 0..=max_column {
        let column_gates: Vec<&PlacedGate> = placed_gates
            .iter()
            .filter(|gate| gate.column == column && gate.wire < qubits)
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
            .filter(|gate| gate.kind == GateKind::DensityMatrixDisplay)
            .collect();
        displays.sort_by(|a, b| a.id.cmp(&b.id));
        for display in displays {
            let span = display.span.clamp(1, 8).min(qubits - display.wire);
            let base_bit = (qubits - display.wire - span) as u32;
            requests.push(ExternalDensityRequest {
                gate_id: display.id,
                column,
                span,
                base_bit,
                control_mask,
                control_value,
            });
        }
    }
    requests
}

pub(super) fn density_requests_json(requests: &[ExternalDensityRequest]) -> String {
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
                request.column,
                request.span,
                request.base_bit,
                request.control_mask,
                request.control_value
            )
        })
        .collect();
    format!("[{}]", entries.join(","))
}

pub(super) fn density_slot_to_gate_id(requests: &[ExternalDensityRequest]) -> Vec<u32> {
    requests.iter().map(|request| request.gate_id).collect()
}

pub(super) fn parse_density_upload_batch(
    message: &str,
    generation: u64,
    slot_to_gate_id: &[u32],
) -> Option<ExternalDensityUploadBatch> {
    if slot_to_gate_id.is_empty() {
        return None;
    }
    parse_density_upload_batch_impl(message, generation, slot_to_gate_id)
}

#[cfg(target_arch = "wasm32")]
fn parse_density_upload_batch_impl(
    message: &str,
    generation: u64,
    slot_to_gate_id: &[u32],
) -> Option<ExternalDensityUploadBatch> {
    use wasm_bindgen::JsCast;

    if slot_to_gate_id.len() > crate::gpu::MAX_DENSITY_SLOTS {
        return None;
    }
    let root = js_sys::JSON::parse(message).ok()?;
    let densities = js_sys::Reflect::get(&root, &wasm_bindgen::JsValue::from_str("densities"))
        .ok()?
        .dyn_into::<js_sys::Array>()
        .ok()?;
    let mut uploads = Vec::new();
    let mut seen_slots = vec![false; slot_to_gate_id.len()];
    for value in densities.iter() {
        let gate_id = js_u32_prop(&value, "gate_id")?;
        let slot_index = slot_to_gate_id.iter().position(|id| *id == gate_id)?;
        if slot_index >= crate::gpu::MAX_DENSITY_SLOTS || seen_slots[slot_index] {
            return None;
        }
        seen_slots[slot_index] = true;
        let span = js_u32_prop(&value, "span")?;
        if span == 0 || span > 8 {
            return None;
        }
        let dim = 1usize << span;
        let cell_count = dim * dim;
        let cells = js_array_prop(&value, "cells")?;
        if cells.length() < cell_count as u32 {
            return None;
        }
        let mut values = Vec::with_capacity(cell_count * 2);
        for index in 0..cell_count as u32 {
            let pair = cells.get(index).dyn_into::<js_sys::Array>().ok()?;
            values.push(pair.get(0).as_f64()? as f32);
            values.push(pair.get(1).as_f64()? as f32);
        }
        let unity = js_optional_f32_prop(&value, "unity").unwrap_or(1.0);
        uploads.push(ExternalDensityUpload {
            slot: slot_index as u32,
            cells: Arc::from(values.into_boxed_slice()),
            meta: [unity, span as f32, 0.0, 0.0],
        });
    }
    if uploads.len() != slot_to_gate_id.len() || seen_slots.iter().any(|seen| !*seen) {
        return None;
    }
    Some(ExternalDensityUploadBatch {
        generation,
        slot_to_gate_id: Arc::from(slot_to_gate_id.to_vec().into_boxed_slice()),
        uploads: Arc::from(uploads.into_boxed_slice()),
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_density_upload_batch_impl(
    _message: &str,
    _generation: u64,
    _slot_to_gate_id: &[u32],
) -> Option<ExternalDensityUploadBatch> {
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
fn js_optional_f32_prop(value: &wasm_bindgen::JsValue, name: &str) -> Option<f32> {
    js_sys::Reflect::get(value, &wasm_bindgen::JsValue::from_str(name))
        .ok()?
        .as_f64()
        .map(|raw| raw as f32)
}

#[cfg(test)]
mod tests {
    use super::{collect_density_requests, density_requests_json, density_slot_to_gate_id};
    use crate::app::PlacedGate;
    use crate::gates::GateKind;
    use crate::qubit_count::QubitCount;

    fn qubit_count(value: usize) -> QubitCount {
        QubitCount::try_new(value).expect("test qubit count must be non-zero")
    }

    #[test]
    fn serializes_density_output_request() {
        let requests = collect_density_requests(
            &[PlacedGate::new(
                2,
                GateKind::DensityMatrixDisplay,
                1,
                0,
                1,
                None,
            )],
            qubit_count(1),
        );

        assert_eq!(
            density_requests_json(&requests),
            r#"[{"gate_id":2,"column":1,"span":1,"base_bit":0,"control_mask":0,"control_value":0}]"#,
        );
    }

    #[test]
    fn serializes_density_span_base_bit() {
        let requests = collect_density_requests(
            &[PlacedGate::new(
                2,
                GateKind::DensityMatrixDisplay,
                1,
                0,
                2,
                None,
            )],
            qubit_count(3),
        );

        assert_eq!(
            density_requests_json(&requests),
            r#"[{"gate_id":2,"column":1,"span":2,"base_bit":1,"control_mask":0,"control_value":0}]"#,
        );
    }

    #[test]
    fn serializes_density_controls() {
        let requests = collect_density_requests(
            &[
                PlacedGate::new(1, GateKind::Control, 2, 0, 1, None),
                PlacedGate::new(2, GateKind::AntiControl, 2, 1, 1, None),
                PlacedGate::new(3, GateKind::DensityMatrixDisplay, 2, 2, 1, None),
            ],
            qubit_count(3),
        );

        assert_eq!(
            density_requests_json(&requests),
            r#"[{"gate_id":3,"column":2,"span":1,"base_bit":0,"control_mask":6,"control_value":4}]"#,
        );
    }

    #[test]
    fn external_density_slot_matches_collection_order() {
        let requests = collect_density_requests(
            &[
                PlacedGate::new(4, GateKind::DensityMatrixDisplay, 1, 0, 1, None),
                PlacedGate::new(3, GateKind::DensityMatrixDisplay, 1, 1, 1, None),
            ],
            qubit_count(2),
        );

        assert_eq!(density_slot_to_gate_id(&requests), vec![3, 4]);
    }
}
