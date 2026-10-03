//! 外部 GPU 実行の現在の状態遷移を固定する。解析器そのものは対象外。
//! ネイティブの解析スタブには検証済みバッチを注入し、調整・後始末は実コードを通す。

use super::super::session::{Acceptance, AcceptedRun, DisplayExpectation, SlotLayout};
use super::super::{ExternalGpuStatus, GpuFailure};
use super::{egui, QniApp};
use crate::app::circuit_history::CircuitRevision;
use crate::app::{CircuitColumnIndex, ExecMode, GateId, PlacedGate, WireIndex};
use crate::gates::{GateKind, GateSpan};
use crate::gpu::*;
use std::sync::Arc;

use super::super::parser_fixtures;

const RUN_ID: u64 = 42;
const RESPONSE: &str = r#"{
    "amplitudes":[{"gate_id":11,"span":1,"ket":[[1,0],[0,0]],"incoherent":[1,0],"quality":1,"phase_lock_index":0}],
    "bloch":[{"gate_id":22,"vector":[0,0,1]}],
    "probability":[{"gate_id":33,"span":1,"probabilities":[1,0]}],
    "densities":[{"gate_id":44,"span":1,"cells":[[1,0],[0,0],[0,0],[0,0]],"unity":1}]
}"#;

#[derive(Debug, PartialEq)]
struct BatchState {
    generation: u64,
    gate_ids: Vec<u32>,
    slots: Vec<u32>,
    values: Vec<(Vec<f32>, Vec<f32>)>,
    meta: Vec<[f32; 4]>,
}

#[derive(Debug, PartialEq)]
enum StatusState {
    Idle,
    Running,
    Completed,
    Failed(GpuFailure),
}

#[derive(Debug, PartialEq)]
struct RunState {
    status: StatusState,
    started_at: Option<f64>,
    refresh_pending: bool,
    uploads: [Option<BatchState>; 4],
    slots: [Vec<u32>; 4],
    run_id: Option<u64>,
    generation: u64,
}

fn state(app: &QniApp) -> RunState {
    let fixture = app.external_gpu.fixture();
    let (run_id, slots) = match &fixture.acceptance {
        Acceptance::Closed => (None, Default::default()),
        Acceptance::Awaiting(accepted) => {
            let slots = match &accepted.expected {
                DisplayExpectation::None => Default::default(),
                DisplayExpectation::Requested(slots) => [
                    slots.amplitude.clone(),
                    slots.bloch.clone(),
                    slots.probability.clone(),
                    slots.density.clone(),
                ],
            };
            (Some(accepted.id), slots)
        }
    };
    RunState {
        status: match &fixture.status {
            ExternalGpuStatus::Idle => StatusState::Idle,
            ExternalGpuStatus::Running => StatusState::Running,
            ExternalGpuStatus::Completed { .. } => StatusState::Completed,
            ExternalGpuStatus::Failed(failure) => StatusState::Failed(failure.clone()),
        },
        started_at: fixture.started_at,
        refresh_pending: fixture.refresh_pending,
        uploads: [
            fixture.displays.amplitude.as_ref().map(|batch| BatchState {
                generation: batch.generation,
                gate_ids: batch.slot_to_gate_id.to_vec(),
                slots: batch.uploads.iter().map(|upload| upload.slot).collect(),
                values: batch
                    .uploads
                    .iter()
                    .map(|upload| (upload.coherent.to_vec(), upload.incoherent.to_vec()))
                    .collect(),
                meta: batch.uploads.iter().map(|upload| upload.meta).collect(),
            }),
            fixture.displays.bloch.as_ref().map(|batch| BatchState {
                generation: batch.generation,
                gate_ids: batch.slot_to_gate_id.to_vec(),
                slots: batch.uploads.iter().map(|upload| upload.slot).collect(),
                values: batch
                    .uploads
                    .iter()
                    .map(|upload| (upload.vector.to_vec(), vec![]))
                    .collect(),
                meta: vec![],
            }),
            fixture
                .displays
                .probability
                .as_ref()
                .map(|batch| BatchState {
                    generation: batch.generation,
                    gate_ids: batch.slot_to_gate_id.to_vec(),
                    slots: batch.uploads.iter().map(|upload| upload.slot).collect(),
                    values: batch
                        .uploads
                        .iter()
                        .map(|upload| (upload.probabilities.as_slice().to_vec(), vec![]))
                        .collect(),
                    meta: vec![],
                }),
            fixture.displays.density.as_ref().map(|batch| BatchState {
                generation: batch.generation,
                gate_ids: batch.slot_to_gate_id.to_vec(),
                slots: batch.uploads.iter().map(|upload| upload.slot).collect(),
                values: batch
                    .uploads
                    .iter()
                    .map(|upload| (upload.cells.to_vec(), vec![]))
                    .collect(),
                meta: batch.uploads.iter().map(|upload| upload.meta).collect(),
            }),
        ],
        slots,
        run_id,
        generation: fixture.generation,
    }
}

fn app() -> (QniApp, egui::Context) {
    parser_fixtures::reset();
    let ctx = egui::Context::default();
    let mut app = QniApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()));
    app.external_gpu.install_scripted_transport();
    app.load_circuit_json_into_editor(r#"{"cols":[]}"#, &ctx);
    // 起動時の保存済み回路ではなく、空の編集可能な回路を履歴の起点にする。
    let (library, _) = crate::app::circuit_library::for_startup(r#"{"cols":[]}"#.into(), true);
    app.library = library;
    app.circuit_revision = CircuitRevision::starting_at(r#"{"cols":[]}"#.into());
    app.exec_mode = ExecMode::Gpu;
    app.external_gpu.edit_fixture(|fixture| {
        fixture.generation = 7;
    });
    (app, ctx)
}

fn amplitude(generation: u64) -> ExternalAmplitudeUploadBatch {
    ExternalAmplitudeUploadBatch {
        generation,
        slot_to_gate_id: Arc::from([11]),
        uploads: Arc::from([ExternalAmplitudeUpload {
            slot: 0,
            coherent: Arc::from([1.0, 0.0, 0.0, 0.0]),
            incoherent: Arc::from([1.0, 0.0]),
            meta: [1.0, 0.0, 1.0, 0.0],
        }]),
    }
}

fn bloch(generation: u64) -> ExternalBlochUploadBatch {
    ExternalBlochUploadBatch {
        generation,
        slot_to_gate_id: Arc::from([22]),
        uploads: Arc::from([ExternalBlochUpload {
            slot: 0,
            vector: [0.0, 0.0, 1.0, 0.0],
        }]),
    }
}

fn probability(generation: u64) -> ExternalProbabilityUploadBatch {
    ExternalProbabilityUploadBatch {
        generation,
        slot_to_gate_id: Arc::from([33]),
        uploads: Arc::from([ExternalProbabilityUpload {
            slot: 0,
            probabilities: ProbabilityDistribution::try_new(Arc::from([1.0, 0.0])).unwrap(),
        }]),
    }
}

fn density(generation: u64) -> ExternalDensityUploadBatch {
    ExternalDensityUploadBatch {
        generation,
        slot_to_gate_id: Arc::from([44]),
        uploads: Arc::from([ExternalDensityUpload {
            slot: 0,
            cells: Arc::from([1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
            meta: [1.0, 1.0, 0.0, 0.0],
        }]),
    }
}

fn set_uploads(app: &mut QniApp, generation: u64) {
    app.external_gpu.edit_fixture(|fixture| {
        fixture.displays.amplitude = Some(amplitude(generation));
        fixture.displays.bloch = Some(bloch(generation));
        fixture.displays.probability = Some(probability(generation));
        fixture.displays.density = Some(density(generation));
    });
}

fn pending(app: &mut QniApp) {
    app.external_gpu.edit_fixture(|fixture| {
        fixture.acceptance = Acceptance::Awaiting(AcceptedRun {
            id: RUN_ID,
            expected: SlotLayout {
                amplitude: vec![11],
                bloch: vec![22],
                probability: vec![33],
                density: vec![44],
            }
            .into(),
        });
        fixture.status = ExternalGpuStatus::Running;
        fixture.started_at = Some(0.0);
    });
}

fn seeded(app: &mut QniApp) {
    pending(app);
    set_uploads(app, 3);
    app.external_gpu.edit_fixture(|fixture| {
        fixture.refresh_pending = true;
    });
}

fn inject_valid_batches(message: &str) {
    parser_fixtures::inject_amplitude(Some((message.into(), amplitude(0))));
    parser_fixtures::inject_bloch(Some((message.into(), bloch(0))));
    parser_fixtures::inject_probability(Some((message.into(), probability(0))));
    parser_fixtures::inject_density(Some((message.into(), density(0))));
}

fn poll(app: &mut QniApp, ctx: &egui::Context, result: Result<String, GpuFailure>) {
    app.external_gpu
        .transport_handle()
        .queue_results(vec![(RUN_ID, result)]);
    app.poll_external_gpu_run(ctx);
}

fn cleared() -> RunState {
    RunState {
        status: StatusState::Idle,
        started_at: None,
        refresh_pending: false,
        uploads: [None, None, None, None],
        slots: [vec![], vec![], vec![], vec![]],
        run_id: None,
        generation: 7,
    }
}

#[test]
fn stale_success_is_discarded_without_changing_current_run() {
    let (mut app, ctx) = app();
    seeded(&mut app);
    let before = state(&app);
    app.external_gpu
        .transport_handle()
        .queue_results(vec![(RUN_ID - 1, Ok(RESPONSE.into()))]);
    app.poll_external_gpu_run(&ctx);
    assert_eq!(
        (state(&app), app.external_gpu.transport_handle().take_one()),
        (before, None)
    );
}

#[test]
fn stale_failure_is_discarded_without_changing_current_run() {
    let (mut app, ctx) = app();
    seeded(&mut app);
    let before = state(&app);
    app.external_gpu
        .transport_handle()
        .queue_results(vec![(RUN_ID - 1, Err(GpuFailure::Http(503)))]);
    app.poll_external_gpu_run(&ctx);
    assert_eq!(
        (state(&app), app.external_gpu.transport_handle().take_one()),
        (before, None)
    );
}

#[test]
fn matching_success_publishes_all_four_batches_and_clears_pending_slots() {
    let (mut app, ctx) = app();
    pending(&mut app);
    // 期待値は固定の応答データから作り、解析結果の世代はアプリが渡す値で上書きする。
    let mut expected = cleared();
    set_uploads(&mut app, 8);
    expected.uploads = state(&app).uploads;
    app.external_gpu.edit_fixture(|fixture| {
        fixture.displays = Default::default();
    });
    expected.status = StatusState::Completed;
    expected.generation = 8;
    inject_valid_batches(RESPONSE);
    poll(&mut app, &ctx, Ok(RESPONSE.into()));
    assert_eq!(
        (
            state(&app),
            app.gpu_plan.needs_recompute_for(app.state_count()),
            app.gpu_plan
                .amplitude_slot(GateId::from_u32(11))
                .map(|slot| slot.as_u32()),
            app.gpu_plan
                .bloch_slot(GateId::from_u32(22))
                .map(|slot| slot.as_u32()),
            app.gpu_plan
                .probability_slot(GateId::from_u32(33))
                .map(|slot| slot.as_u32()),
            app.gpu_plan
                .density_slot(GateId::from_u32(44))
                .map(|slot| slot.as_u32())
        ),
        (expected, false, Some(0), Some(0), Some(0), Some(0))
    );
}

// 各種類で失敗させ、後段で失敗しても先に解析できた種類を公開しないことを固定する。
fn failed_parse(kind: usize, previous_uploads: bool) -> (RunState, RunState) {
    let (mut app, ctx) = app();
    pending(&mut app);
    if previous_uploads {
        set_uploads(&mut app, 3);
    }
    let mut expected = state(&app);
    let labels = ["Amplitude", "Bloch", "Probability", "Density"];
    expected.status = StatusState::Failed(GpuFailure::Other(format!(
        "{} result missing",
        labels[kind]
    )));
    expected.started_at = None;
    expected.slots = [vec![], vec![], vec![], vec![]];
    expected.run_id = None;
    // 成功回数ではない。解析に失敗した場合も世代番号は増える。
    expected.generation = 8;
    // 未要求の ID を含む応答に対する解析失敗を境界で注入する。
    let response = RESPONSE.replace(
        &format!("\"gate_id\":{}", [11, 22, 33, 44][kind]),
        "\"gate_id\":999",
    );
    inject_valid_batches(&response);
    match kind {
        0 => parser_fixtures::inject_amplitude(None),
        1 => parser_fixtures::inject_bloch(None),
        2 => parser_fixtures::inject_probability(None),
        3 => parser_fixtures::inject_density(None),
        _ => unreachable!(),
    }
    poll(&mut app, &ctx, Ok(response));
    (state(&app), expected)
}

#[test]
fn amplitude_parse_failure_applies_nothing_and_advances_generation() {
    let (actual, expected) = failed_parse(0, false);
    assert_eq!(actual, expected);
}

#[test]
fn bloch_parse_failure_does_not_publish_valid_amplitudes() {
    let (actual, expected) = failed_parse(1, false);
    assert_eq!(actual, expected);
}

#[test]
fn probability_parse_failure_does_not_publish_valid_amplitudes_or_bloch() {
    let (actual, expected) = failed_parse(2, false);
    assert_eq!(actual, expected);
}

#[test]
fn density_parse_failure_does_not_publish_any_valid_earlier_kind() {
    let (actual, expected) = failed_parse(3, false);
    assert_eq!(actual, expected);
}

#[test]
fn parse_failure_preserves_preexisting_uploads() {
    let (actual, expected) = failed_parse(2, true);
    assert_eq!(actual, expected);
}

#[test]
fn matching_transport_failure_clears_pending_but_preserves_uploads_and_generation() {
    let (mut app, ctx) = app();
    seeded(&mut app);
    let mut expected = state(&app);
    expected.status = StatusState::Failed(GpuFailure::Http(503));
    expected.started_at = None;
    expected.slots = [vec![], vec![], vec![], vec![]];
    expected.run_id = None;
    poll(&mut app, &ctx, Err(GpuFailure::Http(503)));
    assert_eq!(state(&app), expected);
}

fn add_h(app: &mut QniApp) {
    app.placed_gates.push(PlacedGate::new(
        GateId::from_u32(1),
        GateKind::H,
        CircuitColumnIndex::new(0),
        WireIndex::new(0),
        GateSpan::SINGLE,
        None,
    ));
}

#[test]
fn changed_circuit_commit_invalidates_external_gpu_state() {
    let (mut app, ctx) = app();
    seeded(&mut app);
    add_h(&mut app);
    let committed = app.commit_current_circuit_unchecked(&ctx);
    assert_eq!((committed, state(&app)), (true, cleared()));
}

#[test]
fn identical_circuit_commit_early_returns_without_invalidating_external_gpu_state() {
    let (mut app, ctx) = app();
    // 一度実際の直列化結果を履歴に保存し、同一 JSON の確定を検証する。
    add_h(&mut app);
    app.commit_current_circuit_unchecked(&ctx);
    seeded(&mut app);
    let before = state(&app);
    let committed = app.commit_current_circuit_unchecked(&ctx);
    assert_eq!((committed, state(&app)), (false, before));
}

#[test]
fn loading_circuit_into_editor_invalidates_external_gpu_state() {
    let (mut app, ctx) = app();
    seeded(&mut app);
    app.gpu_plan.mark_clean_for(app.state_count());
    app.load_circuit_json_into_editor(r#"{"cols":[["H"]]}"#, &ctx);
    assert_eq!(
        (
            state(&app),
            app.gpu_plan.needs_recompute_for(app.state_count())
        ),
        (cleared(), true)
    );
}

#[test]
fn replacing_active_circuit_json_invalidates_external_gpu_state() {
    let (mut app, ctx) = app();
    seeded(&mut app);
    app.replace_active_circuit_json_unchecked(r#"{"cols":[["H"]]}"#, &ctx);
    assert_eq!(state(&app), cleared());
}

#[test]
fn undo_invalidates_external_gpu_state_instead_of_restoring_uploads() {
    let (mut app, ctx) = app();
    add_h(&mut app);
    app.commit_current_circuit_unchecked(&ctx);
    seeded(&mut app);
    app.undo_circuit(&ctx);
    assert_eq!((state(&app), app.placed_gates.len()), (cleared(), 0));
}

#[test]
fn redo_invalidates_external_gpu_state_instead_of_restoring_uploads() {
    let (mut app, ctx) = app();
    add_h(&mut app);
    app.commit_current_circuit_unchecked(&ctx);
    app.undo_circuit(&ctx);
    seeded(&mut app);
    app.redo_circuit(&ctx);
    assert_eq!(
        (
            state(&app),
            app.placed_gates
                .iter()
                .map(|gate| gate.kind)
                .collect::<Vec<_>>()
        ),
        (cleared(), vec![GateKind::H])
    );
}

#[test]
fn switching_to_local_through_toggle_clears_results_and_resets_status() {
    let (mut app, ctx) = app();
    seeded(&mut app);
    app.gpu_plan.mark_clean_for(app.state_count());
    app.exec_mode_keyboard_focus = true;
    let input = egui::RawInput {
        events: vec![egui::Event::Key {
            key: egui::Key::ArrowLeft,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
        ..Default::default()
    };
    let _ = ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            app.show_exec_mode_toggle(ui, &app.colors());
        });
    });
    let expected = cleared();
    assert_eq!(
        (
            app.exec_mode,
            state(&app),
            app.gpu_plan.needs_recompute_for(app.state_count())
        ),
        (ExecMode::Local, expected, true)
    );
}

#[test]
fn completion_without_display_outputs_requests_one_shot_gpu_state_refresh() {
    let (mut app, ctx) = app();
    app.external_gpu.edit_fixture(|fixture| {
        fixture.status = ExternalGpuStatus::Running;
        fixture.acceptance = Acceptance::Awaiting(AcceptedRun {
            id: RUN_ID,
            expected: DisplayExpectation::None,
        });
    });
    app.gpu_plan.mark_clean_for(app.state_count());
    // 表示要求がなければ本文は解析しない。これも現在の動作として固定する。
    poll(&mut app, &ctx, Ok("not even JSON".into()));
    let mut expected = cleared();
    expected.status = StatusState::Completed;
    expected.refresh_pending = true;
    assert_eq!(
        (
            state(&app),
            app.gpu_plan.needs_recompute_for(app.state_count())
        ),
        (expected, true)
    );
}

#[test]
fn poll_consumes_only_one_queued_result_when_first_result_matches() {
    let (mut app, ctx) = app();
    seeded(&mut app);
    let mut expected = state(&app);
    expected.status = StatusState::Failed(GpuFailure::Http(503));
    expected.started_at = None;
    expected.slots = [vec![], vec![], vec![], vec![]];
    expected.run_id = None;
    let second = (RUN_ID - 1, Ok(RESPONSE.into()));
    app.external_gpu
        .transport_handle()
        .queue_results(vec![(RUN_ID, Err(GpuFailure::Http(503))), second.clone()]);
    app.poll_external_gpu_run(&ctx);
    assert_eq!(
        (state(&app), app.external_gpu.transport_handle().take_one()),
        (expected, Some(second))
    );
}

#[test]
fn poll_consumes_only_one_queued_result_even_when_first_result_is_stale() {
    let (mut app, ctx) = app();
    seeded(&mut app);
    let before = state(&app);
    let second = (RUN_ID, Err(GpuFailure::Http(503)));
    app.external_gpu
        .transport_handle()
        .queue_results(vec![(RUN_ID - 1, Ok(RESPONSE.into())), second.clone()]);
    app.poll_external_gpu_run(&ctx);
    assert_eq!(
        (state(&app), app.external_gpu.transport_handle().take_one()),
        (before, Some(second))
    );
}

#[path = "lifecycle_extra_tests.rs"]
mod lifecycle_extra_tests;
