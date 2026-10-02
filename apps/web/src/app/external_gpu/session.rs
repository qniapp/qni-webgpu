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
use super::density::{
    collect_density_requests, density_requests_json, density_slot_to_gate_id,
    parse_density_upload_batch,
};
use super::probability::{
    collect_probability_requests, parse_probability_upload_batch, probability_requests_json,
    probability_slot_to_gate_id,
};
use super::test_hooks::take_external_gpu_status_override;
use super::ExecMode;
use super::{qiskit_run_payload_with_display_outputs, ExternalGpuStatus, GpuFailure, Shots};
use crate::app::PlacedGate;
use crate::gates::GateKind;
use crate::gpu::{
    ExternalAmplitudeUploadBatch, ExternalBlochUploadBatch, ExternalDensityUploadBatch,
    ExternalProbabilityUploadBatch,
};
use crate::gpu::{MAX_AMPLITUDE_SLOTS, MAX_BLOCH_SLOTS, MAX_DENSITY_SLOTS, MAX_PROBABILITY_SLOTS};
use crate::qubit_count::{QubitCount, QubitCountError};
use crate::shared::now_seconds;

#[derive(Clone, Default)]
pub(crate) enum Acceptance {
    #[default]
    Closed,
    Awaiting(AcceptedRun),
}

#[derive(Clone)]
pub(crate) struct AcceptedRun {
    pub(crate) id: u64,
    pub(crate) expected: DisplayExpectation,
}

#[derive(Clone)]
pub(crate) enum DisplayExpectation {
    None,
    Requested(SlotLayout),
}

#[derive(Clone, Default)]
pub(crate) struct SlotLayout {
    pub(crate) amplitude: Vec<u32>,
    pub(crate) bloch: Vec<u32>,
    pub(crate) probability: Vec<u32>,
    pub(crate) density: Vec<u32>,
}

impl From<SlotLayout> for DisplayExpectation {
    fn from(slots: SlotLayout) -> Self {
        if slots.amplitude.is_empty()
            && slots.bloch.is_empty()
            && slots.probability.is_empty()
            && slots.density.is_empty()
        {
            Self::None
        } else {
            Self::Requested(slots)
        }
    }
}

struct ExternalGpuRunRequest {
    payload: String,
    amplitude_slot_to_gate_id: Vec<u32>,
    bloch_slot_to_gate_id: Vec<u32>,
    probability_slot_to_gate_id: Vec<u32>,
    density_slot_to_gate_id: Vec<u32>,
}

fn supported_external_controlled_target(kind: GateKind) -> bool {
    matches!(
        kind,
        GateKind::H
            | GateKind::X
            | GateKind::Y
            | GateKind::Z
            | GateKind::SqrtX
            | GateKind::S
            | GateKind::SDagger
            | GateKind::T
            | GateKind::TDagger
            | GateKind::Phase
            | GateKind::Rx
            | GateKind::Ry
            | GateKind::Rz
            | GateKind::Write0
            | GateKind::Write1
    )
}

fn unsupported_external_gpu_gate_for_gates(placed_gates: &[PlacedGate]) -> Option<&'static str> {
    let max_column = placed_gates
        .iter()
        .map(|gate| gate.column.as_usize())
        .max()?;
    for column in 0..=max_column {
        let mut has_control = false;
        let mut unsupported_controlled_target = None;
        for gate in placed_gates
            .iter()
            .filter(|gate| gate.column.as_usize() == column)
        {
            if matches!(gate.kind, GateKind::Control | GateKind::AntiControl) {
                has_control = true;
            } else if supported_external_controlled_target(gate.kind)
                || matches!(
                    gate.kind,
                    GateKind::Measurement
                        | GateKind::QftGate
                        | GateKind::QftDaggerGate
                        | GateKind::Swap
                        | GateKind::AmplitudeDisplay
                        | GateKind::BlochDisplay
                        | GateKind::ProbabilityDisplay
                        | GateKind::DensityMatrixDisplay
                )
            {
            } else if !matches!(gate.kind, GateKind::Spacer) {
                unsupported_controlled_target = Some(gate.kind.label());
            }
        }
        if has_control {
            if let Some(label) = unsupported_controlled_target {
                return Some(label);
            }
        }
    }
    None
}

#[derive(Default)]
pub(crate) struct ExternalGpuSession {
    acceptance: Acceptance,
    presentation: Presentation,
    displays: PublishedDisplays,
    generation: u64,
    refresh: RefreshState,
}
#[derive(Default)]
struct Presentation {
    status: ExternalGpuStatus,
    started_at: Option<f64>,
}
#[derive(Clone, Default)]
pub(crate) struct PublishedDisplays {
    pub(crate) amplitude: Option<ExternalAmplitudeUploadBatch>,
    pub(crate) bloch: Option<ExternalBlochUploadBatch>,
    pub(crate) probability: Option<ExternalProbabilityUploadBatch>,
    pub(crate) density: Option<ExternalDensityUploadBatch>,
}
/// 明示的な外部実行後、状態ベクトル表示を一度だけ WebGPU で更新する。
/// GPU モードでは編集のたびに再計算せず、ローカル容量内の結果を表示する。
#[derive(Default)]
enum RefreshState {
    #[default]
    Clear,
    Pending,
}
pub(crate) enum Invalidation {
    CircuitChanged,
    EnteredLocal,
}
pub(crate) struct CircuitInput<'a> {
    pub(crate) gates: &'a [PlacedGate],
    pub(crate) qubits: Result<QubitCount, QubitCountError>,
}
#[derive(Clone, Copy)]
pub(crate) struct RefreshEnvironment {
    pub(crate) local_available: bool,
    pub(crate) mode: ExecMode,
}
#[must_use]
#[derive(Default)]
pub(crate) struct SessionPublication {
    pub(crate) plan_changes: Vec<PlanChange>,
    pub(crate) repaint: bool,
}
pub(crate) enum PlanChange {
    MarkDirty,
    ReplaceExternalSlots(SlotLayout),
}
pub(crate) struct SessionView<'a> {
    pub(crate) status: &'a ExternalGpuStatus,
    pub(crate) displays: &'a PublishedDisplays,
    pub(crate) refresh_pending: bool,
}
impl ExternalGpuSession {
    pub(crate) fn view(&self) -> SessionView<'_> {
        SessionView {
            status: &self.presentation.status,
            displays: &self.displays,
            refresh_pending: matches!(self.refresh, RefreshState::Pending),
        }
    }
    pub(crate) fn state_refresh_planned(&mut self) {
        self.refresh = RefreshState::Clear;
    }
    pub(crate) fn note_clear_requested(&mut self) {
        self.presentation.status = ExternalGpuStatus::Idle;
    }
    pub(crate) fn invalidate(&mut self, reason: Invalidation) {
        if matches!(reason, Invalidation::CircuitChanged) {
            self.presentation.status = ExternalGpuStatus::Idle;
            self.presentation.started_at = None;
        }
        self.acceptance = Acceptance::Closed;
        self.displays = PublishedDisplays::default();
        self.refresh = RefreshState::Clear;
    }
    pub(crate) fn poll(&mut self, environment: RefreshEnvironment) -> SessionPublication {
        let mut publication = SessionPublication::default();
        if let Some(status) = take_external_gpu_status_override() {
            self.apply_status(status, environment, &mut publication);
        }
        if let Some((run_id, result)) = take_qiskit_run_result() {
            let Acceptance::Awaiting(accepted) = &self.acceptance else {
                return publication;
            };
            if accepted.id != run_id {
                return publication;
            }
            let Acceptance::Awaiting(accepted) =
                std::mem::replace(&mut self.acceptance, Acceptance::Closed)
            else {
                unreachable!()
            };
            let status = match result {
                Ok(message) => self.complete(&message, accepted.expected, &mut publication),
                Err(failure) => {
                    self.presentation.started_at = None;
                    ExternalGpuStatus::Failed(failure)
                }
            };
            self.apply_status(status, environment, &mut publication);
        }
        publication
    }
    fn apply_status(
        &mut self,
        status: ExternalGpuStatus,
        environment: RefreshEnvironment,
        publication: &mut SessionPublication,
    ) {
        match &status {
            ExternalGpuStatus::Idle | ExternalGpuStatus::Failed(_) => {
                self.presentation.started_at = None;
            }
            ExternalGpuStatus::Running => {
                if self.presentation.started_at.is_none() {
                    self.presentation.started_at = Some(now_seconds());
                }
            }
            ExternalGpuStatus::Completed { .. } => {
                self.presentation.started_at = None;
                if self.displays.amplitude.is_none()
                    && self.displays.bloch.is_none()
                    && self.displays.probability.is_none()
                    && self.displays.density.is_none()
                {
                    self.request_state_refresh(environment, publication);
                }
            }
        }
        self.presentation.status = status;
        publication.repaint = true;
    }

    fn take_duration(&mut self) -> Duration {
        let started_at = self
            .presentation
            .started_at
            .take()
            .unwrap_or_else(now_seconds);
        Duration::from_secs_f64((now_seconds() - started_at).max(0.0))
    }

    fn request_state_refresh(
        &mut self,
        environment: RefreshEnvironment,
        publication: &mut SessionPublication,
    ) {
        if !environment.local_available {
            return;
        }
        if environment.mode == ExecMode::Gpu {
            self.refresh = RefreshState::Pending;
        }
        publication.plan_changes.push(PlanChange::MarkDirty);
    }

    pub(crate) fn start(&mut self, input: CircuitInput<'_>, ctx: &egui::Context) -> bool {
        if self.presentation.status.is_running() {
            return false;
        }
        if let Some(gate_name) = unsupported_external_gpu_gate_for_gates(input.gates) {
            self.acceptance = Acceptance::Closed;
            self.presentation.status =
                ExternalGpuStatus::Failed(GpuFailure::UnsupportedGate(gate_name.to_owned()));
            return true;
        }

        let request = match Self::prepare_request(input) {
            Ok(request) => request,
            Err(message) => {
                self.acceptance = Acceptance::Closed;
                self.presentation.status = ExternalGpuStatus::Failed(GpuFailure::Other(message));
                return true;
            }
        };
        if request.amplitude_slot_to_gate_id.len() > MAX_AMPLITUDE_SLOTS {
            self.acceptance = Acceptance::Closed;
            self.presentation.status = ExternalGpuStatus::Failed(GpuFailure::Other(format!(
                "at most {MAX_AMPLITUDE_SLOTS} Amplitude displays"
            )));
            return true;
        }
        if request.bloch_slot_to_gate_id.len() > MAX_BLOCH_SLOTS {
            self.acceptance = Acceptance::Closed;
            self.presentation.status = ExternalGpuStatus::Failed(GpuFailure::Other(format!(
                "at most {MAX_BLOCH_SLOTS} Bloch displays"
            )));
            return true;
        }
        if request.probability_slot_to_gate_id.len() > MAX_PROBABILITY_SLOTS {
            self.acceptance = Acceptance::Closed;
            self.presentation.status = ExternalGpuStatus::Failed(GpuFailure::Other(format!(
                "at most {MAX_PROBABILITY_SLOTS} Probability displays"
            )));
            return true;
        }
        if request.density_slot_to_gate_id.len() > MAX_DENSITY_SLOTS {
            self.acceptance = Acceptance::Closed;
            self.presentation.status = ExternalGpuStatus::Failed(GpuFailure::Other(format!(
                "at most {MAX_DENSITY_SLOTS} Density Matrix displays"
            )));
            return true;
        }
        let expected = DisplayExpectation::from(SlotLayout {
            amplitude: request.amplitude_slot_to_gate_id,
            bloch: request.bloch_slot_to_gate_id,
            probability: request.probability_slot_to_gate_id,
            density: request.density_slot_to_gate_id,
        });
        self.displays.amplitude = None;
        self.displays.bloch = None;
        self.displays.probability = None;
        self.displays.density = None;
        self.presentation.started_at = Some(now_seconds());
        match start_qiskit_run(request.payload, ctx.clone()) {
            Ok(run_id) => {
                self.acceptance = Acceptance::Awaiting(AcceptedRun {
                    id: run_id,
                    expected,
                });
                self.presentation.status = ExternalGpuStatus::Running;
            }
            Err(failure) => {
                self.presentation.started_at = None;
                self.acceptance = Acceptance::Closed;
                self.presentation.status = ExternalGpuStatus::Failed(failure);
            }
        }
        true
    }

    fn complete(
        &mut self,
        message: &str,
        expected: DisplayExpectation,
        publication: &mut SessionPublication,
    ) -> ExternalGpuStatus {
        let duration = self.take_duration();
        let has_display_outputs = matches!(expected, DisplayExpectation::Requested(_));
        let slots = match expected {
            DisplayExpectation::None => SlotLayout::default(),
            DisplayExpectation::Requested(slots) => slots,
        };
        if has_display_outputs {
            self.generation += 1;
        }
        let amplitude_batch = if slots.amplitude.is_empty() {
            None
        } else {
            let Some(batch) =
                parse_amplitude_upload_batch(message, self.generation, &slots.amplitude)
            else {
                return ExternalGpuStatus::Failed(GpuFailure::Other(
                    "Amplitude result missing".to_owned(),
                ));
            };
            Some(batch)
        };
        let bloch_batch = if slots.bloch.is_empty() {
            None
        } else {
            let Some(batch) = parse_bloch_upload_batch(message, self.generation, &slots.bloch)
            else {
                return ExternalGpuStatus::Failed(GpuFailure::Other(
                    "Bloch result missing".to_owned(),
                ));
            };
            Some(batch)
        };
        let probability_batch = if slots.probability.is_empty() {
            None
        } else {
            let Some(batch) =
                parse_probability_upload_batch(message, self.generation, &slots.probability)
            else {
                return ExternalGpuStatus::Failed(GpuFailure::Other(
                    "Probability result missing".to_owned(),
                ));
            };
            Some(batch)
        };
        let density_batch = if slots.density.is_empty() {
            None
        } else {
            let Some(batch) = parse_density_upload_batch(message, self.generation, &slots.density)
            else {
                return ExternalGpuStatus::Failed(GpuFailure::Other(
                    "Density result missing".to_owned(),
                ));
            };
            Some(batch)
        };
        self.displays.amplitude = amplitude_batch;
        self.displays.bloch = bloch_batch;
        self.displays.probability = probability_batch;
        self.displays.density = density_batch;
        if has_display_outputs {
            publication
                .plan_changes
                .push(PlanChange::ReplaceExternalSlots(slots));
        }
        ExternalGpuStatus::Completed { duration }
    }

    fn prepare_request(input: CircuitInput<'_>) -> Result<ExternalGpuRunRequest, String> {
        let qubits = input
            .qubits
            .map_err(|_| "qubit count exceeds external GPU capacity (32)".to_owned())?;
        let columns_json = crate::url_circuit::circuit_columns_to_json(input.gates, qubits);
        let columns =
            crate::simulation_plan::SimulationColumnAnalysis::from_gates(input.gates, qubits);
        let amplitude_requests = collect_amplitude_requests(&columns, qubits);
        let bloch_requests = collect_bloch_requests(&columns);
        let probability_requests = collect_probability_requests(&columns, qubits);
        let density_requests = collect_density_requests(&columns, qubits);
        let amplitudes_json = amplitude_requests_json(&amplitude_requests);
        let bloch_json = bloch_requests_json(&bloch_requests);
        let probability_json = probability_requests_json(&probability_requests);
        let densities_json = density_requests_json(&density_requests);
        Ok(ExternalGpuRunRequest {
            payload: qiskit_run_payload_with_display_outputs(
                qubits.get(),
                &columns_json,
                Shots::DEFAULT,
                &amplitudes_json,
                &bloch_json,
                &probability_json,
                &densities_json,
            ),
            amplitude_slot_to_gate_id: amplitude_slot_to_gate_id(&amplitude_requests),
            bloch_slot_to_gate_id: bloch_slot_to_gate_id(&bloch_requests),
            probability_slot_to_gate_id: probability_slot_to_gate_id(&probability_requests),
            density_slot_to_gate_id: density_slot_to_gate_id(&density_requests),
        })
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[derive(Clone)]
pub(crate) struct SessionFixture {
    pub(crate) acceptance: Acceptance,
    pub(crate) status: ExternalGpuStatus,
    pub(crate) started_at: Option<f64>,
    pub(crate) displays: PublishedDisplays,
    pub(crate) generation: u64,
    pub(crate) refresh_pending: bool,
}
#[cfg(all(test, not(target_arch = "wasm32")))]
impl ExternalGpuSession {
    pub(crate) fn fixture(&self) -> SessionFixture {
        SessionFixture {
            acceptance: self.acceptance.clone(),
            status: self.presentation.status.clone(),
            started_at: self.presentation.started_at,
            displays: self.displays.clone(),
            generation: self.generation,
            refresh_pending: self.view().refresh_pending,
        }
    }
    pub(crate) fn edit_fixture(&mut self, edit: impl FnOnce(&mut SessionFixture)) {
        let mut fixture = self.fixture();
        edit(&mut fixture);
        self.acceptance = fixture.acceptance;
        self.presentation = Presentation {
            status: fixture.status,
            started_at: fixture.started_at,
        };
        self.displays = fixture.displays;
        self.generation = fixture.generation;
        self.refresh = if fixture.refresh_pending {
            RefreshState::Pending
        } else {
            RefreshState::Clear
        };
    }
}
#[cfg(test)]
mod tests {
    use super::{DisplayExpectation, SlotLayout};

    #[test]
    fn display_expectation_is_requested_when_any_slot_kind_is_nonempty() {
        let layouts = [
            SlotLayout::default(),
            SlotLayout {
                amplitude: vec![11],
                ..Default::default()
            },
            SlotLayout {
                bloch: vec![22],
                ..Default::default()
            },
            SlotLayout {
                probability: vec![33],
                ..Default::default()
            },
            SlotLayout {
                density: vec![44],
                ..Default::default()
            },
        ];
        assert_eq!(
            layouts.map(|slots| matches!(slots.into(), DisplayExpectation::Requested(_))),
            [false, true, true, true, true]
        );
    }
}
#[cfg(test)]
mod gate_tests {
    use super::unsupported_external_gpu_gate_for_gates;
    use crate::app::PlacedGate;
    use crate::gates::GateKind;

    #[test]
    fn external_gpu_accepts_write0_gate() {
        let gates = [PlacedGate::new(
            crate::app::GateId::from_u32(1),
            GateKind::Write0,
            crate::app::CircuitColumnIndex::new(0),
            crate::app::WireIndex::new(0),
            crate::gates::GateSpan::SINGLE,
            None,
        )];

        assert_eq!(unsupported_external_gpu_gate_for_gates(&gates), None);
    }

    #[test]
    fn external_gpu_accepts_write1_gate() {
        let gates = [PlacedGate::new(
            crate::app::GateId::from_u32(1),
            GateKind::Write1,
            crate::app::CircuitColumnIndex::new(0),
            crate::app::WireIndex::new(0),
            crate::gates::GateSpan::SINGLE,
            None,
        )];

        assert_eq!(unsupported_external_gpu_gate_for_gates(&gates), None);
    }

    #[test]
    fn external_gpu_accepts_controlled_write0_gate() {
        let gates = [
            PlacedGate::new(
                crate::app::GateId::from_u32(1),
                GateKind::Control,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(0),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
            PlacedGate::new(
                crate::app::GateId::from_u32(2),
                GateKind::Write0,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(1),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
        ];

        assert_eq!(unsupported_external_gpu_gate_for_gates(&gates), None);
    }

    #[test]
    fn external_gpu_accepts_anti_controlled_write1_gate() {
        let gates = [
            PlacedGate::new(
                crate::app::GateId::from_u32(1),
                GateKind::AntiControl,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(0),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
            PlacedGate::new(
                crate::app::GateId::from_u32(2),
                GateKind::Write1,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(1),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
        ];

        assert_eq!(unsupported_external_gpu_gate_for_gates(&gates), None);
    }

    #[test]
    fn external_gpu_accepts_swap_gate() {
        let gates = [
            PlacedGate::new(
                crate::app::GateId::from_u32(1),
                GateKind::Swap,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(0),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
            PlacedGate::new(
                crate::app::GateId::from_u32(2),
                GateKind::Swap,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(1),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
        ];

        assert_eq!(unsupported_external_gpu_gate_for_gates(&gates), None);
    }

    #[test]
    fn external_gpu_accepts_spacer_gate() {
        let gates = [PlacedGate::new(
            crate::app::GateId::from_u32(1),
            GateKind::Spacer,
            crate::app::CircuitColumnIndex::new(0),
            crate::app::WireIndex::new(0),
            crate::gates::GateSpan::SINGLE,
            None,
        )];

        assert_eq!(unsupported_external_gpu_gate_for_gates(&gates), None);
    }

    #[test]
    fn external_gpu_accepts_measurement_gate() {
        let gates = [PlacedGate::new(
            crate::app::GateId::from_u32(1),
            GateKind::Measurement,
            crate::app::CircuitColumnIndex::new(0),
            crate::app::WireIndex::new(0),
            crate::gates::GateSpan::SINGLE,
            None,
        )];

        assert_eq!(unsupported_external_gpu_gate_for_gates(&gates), None);
    }

    #[test]
    fn external_gpu_accepts_anti_controlled_x_gate() {
        let gates = [
            PlacedGate::new(
                crate::app::GateId::from_u32(1),
                GateKind::AntiControl,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(0),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
            PlacedGate::new(
                crate::app::GateId::from_u32(2),
                GateKind::X,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(1),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
        ];

        assert_eq!(unsupported_external_gpu_gate_for_gates(&gates), None);
    }

    #[test]
    fn external_gpu_accepts_lone_control_column() {
        let gates = [PlacedGate::new(
            crate::app::GateId::from_u32(1),
            GateKind::Control,
            crate::app::CircuitColumnIndex::new(0),
            crate::app::WireIndex::new(0),
            crate::gates::GateSpan::SINGLE,
            None,
        )];

        assert_eq!(unsupported_external_gpu_gate_for_gates(&gates), None);
    }

    #[test]
    fn external_gpu_accepts_control_only_column() {
        let gates = [
            PlacedGate::new(
                crate::app::GateId::from_u32(1),
                GateKind::Control,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(0),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
            PlacedGate::new(
                crate::app::GateId::from_u32(2),
                GateKind::Control,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(1),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
        ];

        assert_eq!(unsupported_external_gpu_gate_for_gates(&gates), None);
    }

    #[test]
    fn external_gpu_accepts_anti_control_only_column() {
        let gates = [PlacedGate::new(
            crate::app::GateId::from_u32(1),
            GateKind::AntiControl,
            crate::app::CircuitColumnIndex::new(0),
            crate::app::WireIndex::new(0),
            crate::gates::GateSpan::SINGLE,
            None,
        )];

        assert_eq!(unsupported_external_gpu_gate_for_gates(&gates), None);
    }

    #[test]
    fn external_gpu_accepts_anti_controlled_h_gate() {
        let gates = [
            PlacedGate::new(
                crate::app::GateId::from_u32(1),
                GateKind::AntiControl,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(0),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
            PlacedGate::new(
                crate::app::GateId::from_u32(2),
                GateKind::H,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(1),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
        ];

        assert_eq!(unsupported_external_gpu_gate_for_gates(&gates), None);
    }

    #[test]
    fn external_gpu_accepts_controlled_sqrt_x_gate() {
        let gates = [
            PlacedGate::new(
                crate::app::GateId::from_u32(1),
                GateKind::Control,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(0),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
            PlacedGate::new(
                crate::app::GateId::from_u32(2),
                GateKind::SqrtX,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(1),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
        ];

        assert_eq!(unsupported_external_gpu_gate_for_gates(&gates), None);
    }

    #[test]
    fn external_gpu_accepts_controlled_measurement_gate() {
        let gates = [
            PlacedGate::new(
                crate::app::GateId::from_u32(1),
                GateKind::Control,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(0),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
            PlacedGate::new(
                crate::app::GateId::from_u32(2),
                GateKind::Measurement,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(1),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
        ];

        assert_eq!(unsupported_external_gpu_gate_for_gates(&gates), None);
    }

    #[test]
    fn external_gpu_accepts_anti_controlled_measurement_gate() {
        let gates = [
            PlacedGate::new(
                crate::app::GateId::from_u32(1),
                GateKind::AntiControl,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(0),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
            PlacedGate::new(
                crate::app::GateId::from_u32(2),
                GateKind::Measurement,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(1),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
        ];

        assert_eq!(unsupported_external_gpu_gate_for_gates(&gates), None);
    }

    #[test]
    fn external_gpu_accepts_qft_gate() {
        let gates = [PlacedGate::new(
            crate::app::GateId::from_u32(1),
            GateKind::QftGate,
            crate::app::CircuitColumnIndex::new(0),
            crate::app::WireIndex::new(0),
            crate::gates::GateSpan::try_new(2).unwrap(),
            None,
        )];

        assert_eq!(unsupported_external_gpu_gate_for_gates(&gates), None);
    }

    #[test]
    fn external_gpu_accepts_qft_dagger_gate() {
        let gates = [PlacedGate::new(
            crate::app::GateId::from_u32(1),
            GateKind::QftDaggerGate,
            crate::app::CircuitColumnIndex::new(0),
            crate::app::WireIndex::new(0),
            crate::gates::GateSpan::try_new(2).unwrap(),
            None,
        )];

        assert_eq!(unsupported_external_gpu_gate_for_gates(&gates), None);
    }

    #[test]
    fn external_gpu_accepts_controlled_qft_gate() {
        let gates = [
            PlacedGate::new(
                crate::app::GateId::from_u32(1),
                GateKind::Control,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(0),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
            PlacedGate::new(
                crate::app::GateId::from_u32(2),
                GateKind::QftGate,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(1),
                crate::gates::GateSpan::try_new(2).unwrap(),
                None,
            ),
        ];

        assert_eq!(unsupported_external_gpu_gate_for_gates(&gates), None);
    }

    #[test]
    fn external_gpu_accepts_controlled_qft_dagger_gate() {
        let gates = [
            PlacedGate::new(
                crate::app::GateId::from_u32(1),
                GateKind::AntiControl,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(0),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
            PlacedGate::new(
                crate::app::GateId::from_u32(2),
                GateKind::QftDaggerGate,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(1),
                crate::gates::GateSpan::try_new(2).unwrap(),
                None,
            ),
        ];

        assert_eq!(unsupported_external_gpu_gate_for_gates(&gates), None);
    }

    #[test]
    fn external_gpu_accepts_controlled_swap_gate() {
        let gates = [
            PlacedGate::new(
                crate::app::GateId::from_u32(1),
                GateKind::Control,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(0),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
            PlacedGate::new(
                crate::app::GateId::from_u32(2),
                GateKind::Swap,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(1),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
            PlacedGate::new(
                crate::app::GateId::from_u32(3),
                GateKind::Swap,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(2),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
        ];

        assert_eq!(unsupported_external_gpu_gate_for_gates(&gates), None);
    }

    #[test]
    fn external_gpu_accepts_controlled_density_display() {
        let gates = [
            PlacedGate::new(
                crate::app::GateId::from_u32(1),
                GateKind::Control,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(0),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
            PlacedGate::new(
                crate::app::GateId::from_u32(2),
                GateKind::DensityMatrixDisplay,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(1),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
        ];

        assert_eq!(unsupported_external_gpu_gate_for_gates(&gates), None);
    }
}
