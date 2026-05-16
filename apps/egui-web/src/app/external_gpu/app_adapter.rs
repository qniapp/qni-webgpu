use std::time::Duration;

use eframe::egui;

use super::client::{start_qiskit_run, take_qiskit_run_result};
use super::payload::qiskit_run_payload;
use super::status::{ExternalGpuStatus, GpuFailure};
use super::test_hooks::{take_external_gpu_status_override, wire_external_gpu_test_hooks};
use super::{ExecMode, QniApp};
use crate::app::circuit_library;
use crate::gates::GateKind;
use crate::shared::now_seconds;

impl QniApp {
    pub(crate) fn external_gpu_status(&self) -> &ExternalGpuStatus {
        &self.external_gpu_status
    }

    pub(crate) fn poll_external_gpu_run(&mut self, ctx: &egui::Context) {
        if let Some(status) = take_external_gpu_status_override() {
            self.apply_external_gpu_status(status, ctx);
        }
        if let Some(result) = take_qiskit_run_result() {
            let status = match result {
                Ok(_message) => {
                    let duration = self.take_external_gpu_duration();
                    ExternalGpuStatus::Completed { duration }
                }
                Err(failure) => {
                    self.external_gpu_started_at = None;
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
                self.request_external_gpu_state_panel_refresh();
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
            self.external_gpu_status =
                ExternalGpuStatus::Failed(GpuFailure::UnsupportedGate(gate_name.to_owned()));
            ctx.request_repaint();
            return;
        }

        let payload = self.external_gpu_run_payload();
        self.external_gpu_started_at = Some(now_seconds());
        match start_qiskit_run(payload, ctx.clone()) {
            Ok(()) => self.external_gpu_status = ExternalGpuStatus::Running,
            Err(failure) => {
                self.external_gpu_started_at = None;
                self.external_gpu_status = ExternalGpuStatus::Failed(failure);
            }
        }
        ctx.request_repaint();
    }

    fn unsupported_external_gpu_gate(&self) -> Option<&'static str> {
        for gate in &self.placed_gates {
            let name = match gate.kind {
                GateKind::AntiControl => Some("Anti-control"),
                GateKind::BlochDisplay => Some("Bloch"),
                GateKind::Measurement => Some("Measurement"),
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
            for gate in self
                .placed_gates
                .iter()
                .filter(|gate| gate.column == column)
            {
                if gate.kind == GateKind::Control {
                    has_control = true;
                } else if gate.kind == GateKind::X {
                    has_x_target = true;
                } else if has_control {
                    return Some(gate.kind.label());
                }
            }
            if has_control && !has_x_target {
                return Some("Control");
            }
        }
        None
    }

    fn external_gpu_run_payload(&self) -> String {
        let qubits = self.external_execution_qubits();
        let columns_json = crate::url_circuit::circuit_columns_to_json(&self.placed_gates, qubits);
        qiskit_run_payload(qubits, &columns_json, 1024)
    }

    pub(crate) fn wire_test_hooks(ctx: &egui::Context) {
        wire_external_gpu_test_hooks(ctx);
        circuit_library::wire_test_hooks(ctx);
    }
}
