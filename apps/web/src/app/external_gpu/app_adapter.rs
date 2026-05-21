use std::time::Duration;

use eframe::egui;

use super::amplitude::{
    amplitude_requests_json, amplitude_slot_to_gate_id, collect_amplitude_requests,
    parse_amplitude_upload_batch,
};
use super::bloch::{
    bloch_requests_json, bloch_slot_to_gate_id, collect_bloch_requests, parse_bloch_upload_batch,
};
use super::client::{start_qiskit_run, take_qiskit_run_result};
use super::probability::{
    collect_probability_requests, parse_probability_upload_batch, probability_requests_json,
    probability_slot_to_gate_id,
};
use super::test_hooks::{take_external_gpu_status_override, wire_external_gpu_test_hooks};
use super::{qiskit_run_payload_with_display_outputs, ExternalGpuStatus, GpuFailure};
use super::{ExecMode, QniApp};
use crate::app::circuit_library;
use crate::gates::GateKind;
use crate::gpu::{MAX_AMPLITUDE_SLOTS, MAX_BLOCH_SLOTS, MAX_PROBABILITY_SLOTS};
use crate::shared::now_seconds;

struct ExternalGpuRunRequest {
    payload: String,
    amplitude_slot_to_gate_id: Vec<u32>,
    bloch_slot_to_gate_id: Vec<u32>,
    probability_slot_to_gate_id: Vec<u32>,
}

impl QniApp {
    pub(crate) fn external_gpu_status(&self) -> &ExternalGpuStatus {
        &self.external_gpu_status
    }

    pub(crate) fn poll_external_gpu_run(&mut self, ctx: &egui::Context) {
        if let Some(status) = take_external_gpu_status_override() {
            self.apply_external_gpu_status(status, ctx);
        }
        if let Some((run_id, result)) = take_qiskit_run_result() {
            if self.pending_external_gpu_run_id != Some(run_id) {
                return;
            }
            self.pending_external_gpu_run_id = None;
            let status = match result {
                Ok(message) => self.complete_external_gpu_run(&message),
                Err(failure) => {
                    self.external_gpu_started_at = None;
                    self.pending_external_amplitude_slots.clear();
                    self.pending_external_bloch_slots.clear();
                    self.pending_external_probability_slots.clear();
                    ExternalGpuStatus::Failed(failure)
                }
            };
            self.apply_external_gpu_status(status, ctx);
        }
    }

    fn apply_external_gpu_status(&mut self, status: ExternalGpuStatus, ctx: &egui::Context) {
        match &status {
            ExternalGpuStatus::Idle | ExternalGpuStatus::Failed(_) => {
                self.external_gpu_started_at = None;
            }
            ExternalGpuStatus::Running => {
                if self.external_gpu_started_at.is_none() {
                    self.external_gpu_started_at = Some(now_seconds());
                }
            }
            ExternalGpuStatus::Completed { .. } => {
                self.external_gpu_started_at = None;
                if self.external_gpu_amplitude_uploads.is_none()
                    && self.external_gpu_bloch_uploads.is_none()
                    && self.external_gpu_probability_uploads.is_none()
                {
                    self.request_external_gpu_state_panel_refresh();
                }
            }
        }
        self.external_gpu_status = status;
        ctx.request_repaint();
    }

    fn take_external_gpu_duration(&mut self) -> Duration {
        let started_at = self
            .external_gpu_started_at
            .take()
            .unwrap_or_else(now_seconds);
        Duration::from_secs_f64((now_seconds() - started_at).max(0.0))
    }

    fn request_external_gpu_state_panel_refresh(&mut self) {
        if !self.local_exec_mode_available() {
            return;
        }
        if self.exec_mode == ExecMode::Gpu {
            self.external_gpu_state_refresh_pending = true;
        }
        self.gpu_plan.mark_dirty();
    }

    pub(crate) fn start_external_gpu_run(&mut self, ctx: &egui::Context) {
        if self.external_gpu_status.is_running() {
            return;
        }
        if let Some(gate_name) = self.unsupported_external_gpu_gate() {
            self.pending_external_gpu_run_id = None;
            self.pending_external_amplitude_slots.clear();
            self.pending_external_bloch_slots.clear();
            self.pending_external_probability_slots.clear();
            self.external_gpu_status =
                ExternalGpuStatus::Failed(GpuFailure::UnsupportedGate(gate_name.to_owned()));
            ctx.request_repaint();
            return;
        }

        let request = self.external_gpu_run_request();
        if request.amplitude_slot_to_gate_id.len() > MAX_AMPLITUDE_SLOTS {
            self.pending_external_gpu_run_id = None;
            self.pending_external_amplitude_slots.clear();
            self.pending_external_bloch_slots.clear();
            self.pending_external_probability_slots.clear();
            self.external_gpu_status = ExternalGpuStatus::Failed(GpuFailure::Other(format!(
                "at most {MAX_AMPLITUDE_SLOTS} Amplitude displays"
            )));
            ctx.request_repaint();
            return;
        }
        if request.bloch_slot_to_gate_id.len() > MAX_BLOCH_SLOTS {
            self.pending_external_gpu_run_id = None;
            self.pending_external_amplitude_slots.clear();
            self.pending_external_bloch_slots.clear();
            self.pending_external_probability_slots.clear();
            self.external_gpu_status = ExternalGpuStatus::Failed(GpuFailure::Other(format!(
                "at most {MAX_BLOCH_SLOTS} Bloch displays"
            )));
            ctx.request_repaint();
            return;
        }
        if request.probability_slot_to_gate_id.len() > MAX_PROBABILITY_SLOTS {
            self.pending_external_gpu_run_id = None;
            self.pending_external_amplitude_slots.clear();
            self.pending_external_bloch_slots.clear();
            self.pending_external_probability_slots.clear();
            self.external_gpu_status = ExternalGpuStatus::Failed(GpuFailure::Other(format!(
                "at most {MAX_PROBABILITY_SLOTS} Probability displays"
            )));
            ctx.request_repaint();
            return;
        }
        self.pending_external_amplitude_slots = request.amplitude_slot_to_gate_id;
        self.pending_external_bloch_slots = request.bloch_slot_to_gate_id;
        self.pending_external_probability_slots = request.probability_slot_to_gate_id;
        self.external_gpu_amplitude_uploads = None;
        self.external_gpu_bloch_uploads = None;
        self.external_gpu_probability_uploads = None;
        self.external_gpu_started_at = Some(now_seconds());
        match start_qiskit_run(request.payload, ctx.clone()) {
            Ok(run_id) => {
                self.pending_external_gpu_run_id = Some(run_id);
                self.external_gpu_status = ExternalGpuStatus::Running;
            }
            Err(failure) => {
                self.external_gpu_started_at = None;
                self.pending_external_amplitude_slots.clear();
                self.pending_external_bloch_slots.clear();
                self.pending_external_probability_slots.clear();
                self.pending_external_gpu_run_id = None;
                self.external_gpu_status = ExternalGpuStatus::Failed(failure);
            }
        }
        ctx.request_repaint();
    }

    fn unsupported_external_gpu_gate(&self) -> Option<&'static str> {
        for gate in &self.placed_gates {
            let name = match gate.kind {
                GateKind::AntiControl => Some("Anti-control"),
                GateKind::BlochDisplay => None,
                GateKind::Measurement => Some("Measurement"),
                GateKind::ProbabilityDisplay => None,
                GateKind::AmplitudeDisplay => None,
                GateKind::Spacer => Some("Spacer"),
                GateKind::Write0 => Some("|0⟩"),
                GateKind::Write1 => Some("|1⟩"),
                GateKind::Swap => Some("Swap"),
                GateKind::QftGate => Some("QFT"),
                GateKind::QftDaggerGate => Some("QFT†"),
                _ => None,
            };
            if let Some(name) = name {
                return Some(name);
            }
        }

        let max_column = self.placed_gates.iter().map(|gate| gate.column).max()?;
        for column in 0..=max_column {
            let mut has_control = false;
            let mut has_x_target = false;
            let mut has_readonly_display = false;
            let mut non_x_target = None;
            for gate in self
                .placed_gates
                .iter()
                .filter(|gate| gate.column == column)
            {
                if gate.kind == GateKind::Control {
                    has_control = true;
                } else if gate.kind == GateKind::X {
                    has_x_target = true;
                } else if matches!(
                    gate.kind,
                    GateKind::AmplitudeDisplay
                        | GateKind::BlochDisplay
                        | GateKind::ProbabilityDisplay
                ) {
                    has_readonly_display = true;
                } else if !matches!(gate.kind, GateKind::Spacer) {
                    non_x_target = Some(gate.kind.label());
                }
            }
            if has_control {
                if let Some(label) = non_x_target {
                    return Some(label);
                }
                if !has_x_target && !has_readonly_display {
                    return Some("Control");
                }
            }
        }
        None
    }

    fn complete_external_gpu_run(&mut self, message: &str) -> ExternalGpuStatus {
        let duration = self.take_external_gpu_duration();
        let has_display_outputs = !self.pending_external_amplitude_slots.is_empty()
            || !self.pending_external_bloch_slots.is_empty()
            || !self.pending_external_probability_slots.is_empty();
        if has_display_outputs {
            self.external_gpu_display_generation += 1;
        }
        let amplitude_batch = if self.pending_external_amplitude_slots.is_empty() {
            None
        } else {
            let Some(batch) = parse_amplitude_upload_batch(
                message,
                self.external_gpu_display_generation,
                &self.pending_external_amplitude_slots,
            ) else {
                self.pending_external_amplitude_slots.clear();
                self.pending_external_bloch_slots.clear();
                self.pending_external_probability_slots.clear();
                return ExternalGpuStatus::Failed(GpuFailure::Other(
                    "Amplitude result missing".to_owned(),
                ));
            };
            Some(batch)
        };
        let bloch_batch = if self.pending_external_bloch_slots.is_empty() {
            None
        } else {
            let Some(batch) = parse_bloch_upload_batch(
                message,
                self.external_gpu_display_generation,
                &self.pending_external_bloch_slots,
            ) else {
                self.pending_external_amplitude_slots.clear();
                self.pending_external_bloch_slots.clear();
                self.pending_external_probability_slots.clear();
                return ExternalGpuStatus::Failed(GpuFailure::Other(
                    "Bloch result missing".to_owned(),
                ));
            };
            Some(batch)
        };
        let probability_batch = if self.pending_external_probability_slots.is_empty() {
            None
        } else {
            let Some(batch) = parse_probability_upload_batch(
                message,
                self.external_gpu_display_generation,
                &self.pending_external_probability_slots,
            ) else {
                self.pending_external_amplitude_slots.clear();
                self.pending_external_bloch_slots.clear();
                self.pending_external_probability_slots.clear();
                return ExternalGpuStatus::Failed(GpuFailure::Other(
                    "Probability result missing".to_owned(),
                ));
            };
            Some(batch)
        };
        self.external_gpu_amplitude_uploads = amplitude_batch;
        self.external_gpu_bloch_uploads = bloch_batch;
        self.external_gpu_probability_uploads = probability_batch;
        if has_display_outputs {
            self.gpu_plan.replace_external_display_slots(
                &self.pending_external_amplitude_slots,
                &self.pending_external_bloch_slots,
                &self.pending_external_probability_slots,
                self.state_count(),
            );
        }
        self.pending_external_amplitude_slots.clear();
        self.pending_external_bloch_slots.clear();
        self.pending_external_probability_slots.clear();
        ExternalGpuStatus::Completed { duration }
    }

    fn external_gpu_run_request(&self) -> ExternalGpuRunRequest {
        let qubits = self.external_execution_qubits();
        let columns_json = crate::url_circuit::circuit_columns_to_json(&self.placed_gates, qubits);
        let amplitude_requests = collect_amplitude_requests(&self.placed_gates, qubits);
        let bloch_requests = collect_bloch_requests(&self.placed_gates, qubits);
        let probability_requests = collect_probability_requests(&self.placed_gates, qubits);
        let amplitudes_json = amplitude_requests_json(&amplitude_requests);
        let bloch_json = bloch_requests_json(&bloch_requests);
        let probability_json = probability_requests_json(&probability_requests);
        ExternalGpuRunRequest {
            payload: qiskit_run_payload_with_display_outputs(
                qubits,
                &columns_json,
                1024,
                &amplitudes_json,
                &bloch_json,
                &probability_json,
            ),
            amplitude_slot_to_gate_id: amplitude_slot_to_gate_id(&amplitude_requests),
            bloch_slot_to_gate_id: bloch_slot_to_gate_id(&bloch_requests),
            probability_slot_to_gate_id: probability_slot_to_gate_id(&probability_requests),
        }
    }

    pub(crate) fn wire_test_hooks(ctx: &egui::Context) {
        wire_external_gpu_test_hooks(ctx);
        circuit_library::wire_test_hooks(ctx);
    }
}
