//! 次の所有権移動で変わり得る開始・時刻・計画更新の経路を固定する。

use super::*;
use crate::shared::test_clock::with_times;
use std::time::Duration;

fn gate(kind: GateKind, id: u32, column: usize, wire: usize) -> PlacedGate {
    PlacedGate::new(
        GateId::from_u32(id),
        kind,
        CircuitColumnIndex::new(column),
        WireIndex::new(wire),
        GateSpan::SINGLE,
        None,
    )
}

fn ready_to_start(app: &mut QniApp) {
    seeded(app);
    app.external_gpu_status = ExternalGpuStatus::Completed {
        duration: Duration::from_secs(1),
    };
}

fn validation_failure(
    mut app: QniApp,
    ctx: &egui::Context,
    message: String,
) -> (RunState, RunState) {
    ready_to_start(&mut app);
    let mut expected = state(&app);
    expected.status = StatusState::Failed(GpuFailure::Other(message));
    expected.run_id = None;
    expected.slots = Default::default();
    // 開始前の検査失敗は旧 uploads・開始時刻・refresh・世代を保持する。
    // 時刻取得にも到達しない。開始時刻が残る現状を修正せず固定する。
    with_times(&[], || app.start_external_gpu_run(ctx));
    (state(&app), expected)
}

#[test]
fn qubit_capacity_validation_failure_preserves_previous_presentation_and_uploads() {
    let (mut app, ctx) = app();
    app.placed_gates.push(gate(GateKind::H, 1, 0, 32));
    let (actual, expected) = validation_failure(
        app,
        &ctx,
        "qubit count exceeds external GPU capacity (32)".into(),
    );
    assert_eq!(actual, expected);
}

fn display_capacity_failure(kind: GateKind, max: usize, label: &str) -> (RunState, RunState) {
    let (mut app, ctx) = app();
    app.placed_gates = (0..=max)
        .map(|i| gate(kind, i as u32 + 100, i, 0))
        .collect();
    validation_failure(app, &ctx, format!("at most {max} {label} displays"))
}

#[test]
fn amplitude_capacity_validation_failure_preserves_previous_results() {
    let (actual, expected) =
        display_capacity_failure(GateKind::AmplitudeDisplay, MAX_AMPLITUDE_SLOTS, "Amplitude");
    assert_eq!(actual, expected);
}

#[test]
fn bloch_capacity_validation_failure_preserves_previous_results() {
    let (actual, expected) =
        display_capacity_failure(GateKind::BlochDisplay, MAX_BLOCH_SLOTS, "Bloch");
    assert_eq!(actual, expected);
}

#[test]
fn probability_capacity_validation_failure_preserves_previous_results() {
    let (actual, expected) = display_capacity_failure(
        GateKind::ProbabilityDisplay,
        MAX_PROBABILITY_SLOTS,
        "Probability",
    );
    assert_eq!(actual, expected);
}

#[test]
fn density_capacity_validation_failure_preserves_previous_results() {
    let (actual, expected) = display_capacity_failure(
        GateKind::DensityMatrixDisplay,
        MAX_DENSITY_SLOTS,
        "Density Matrix",
    );
    assert_eq!(actual, expected);
}

#[test]
fn synchronous_start_failure_clears_old_uploads_and_pending_but_keeps_refresh_and_generation() {
    let (mut app, ctx) = app();
    ready_to_start(&mut app);
    app.placed_gates
        .push(gate(GateKind::BlochDisplay, 22, 0, 0));
    let mut expected = cleared();
    expected.status = StatusState::Failed(GpuFailure::Other(
        "Qiskit backend fetch is only available in wasm".into(),
    ));
    expected.refresh_pending = true;
    let (_, remaining) = with_times(&[17.0], || app.start_external_gpu_run(&ctx));
    assert_eq!((state(&app), remaining), (expected, 0));
}

#[test]
fn start_while_running_is_noop_even_with_invalid_qubit_count() {
    let (mut app, ctx) = app();
    seeded(&mut app);
    app.placed_gates.push(gate(GateKind::H, 1, 0, 32));
    let before = state(&app);
    let (_, remaining) = with_times(&[], || app.start_external_gpu_run(&ctx));
    assert_eq!((state(&app), remaining), (before, 0));
}

#[test]
fn toolbar_clear_on_identical_empty_circuit_changes_only_status_in_run_state() {
    let (mut app, ctx) = app();
    // 空回路の直列化結果を先に確定し、Clear 後の履歴確定を早期 return させる。
    app.commit_current_circuit_unchecked(&ctx);
    seeded(&mut app);
    let mut expected = state(&app);
    expected.status = StatusState::Idle;
    // 同一回路の確定では受理権も開始時刻も残る。今回これを修正しない。
    let mut position = egui::Pos2::ZERO;
    let draw = |app: &mut QniApp, ctx: &egui::Context, position: &mut egui::Pos2| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.horizontal(|ui| {
                *position = ui.cursor().min + egui::vec2(96.0, 16.0);
                app.show_test_edit_utilities(ui);
            });
        });
    };
    let _ = ctx.run(Default::default(), |ctx| draw(&mut app, ctx, &mut position));
    for pressed in [true, false] {
        let input = egui::RawInput {
            events: vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| draw(&mut app, ctx, &mut position));
    }
    assert_eq!(state(&app), expected);
}

#[test]
fn completed_override_then_matching_display_success_leaves_plan_clean_and_refresh_pending() {
    let (mut app, ctx) = app();
    pending(&mut app);
    app.gpu_plan.mark_clean_for(app.state_count());
    let mut expected = cleared();
    set_uploads(&mut app, 8);
    expected.uploads = state(&app).uploads;
    app.external_gpu_amplitude_uploads = None;
    app.external_gpu_bloch_uploads = None;
    app.external_gpu_probability_uploads = None;
    app.external_gpu_density_uploads = None;
    expected.status = StatusState::Completed;
    expected.generation = 8;
    // 表示成功後にも override の更新要求が残る現状を固定する。
    expected.refresh_pending = true;
    inject_valid_batches(RESPONSE);
    super::super::super::test_hooks::inject_external_gpu_status(ExternalGpuStatus::Completed {
        duration: Duration::from_secs(99),
    });
    // override が開始時刻を消すので、完了処理は二回時刻を取得する。
    let (_, remaining) = with_times(&[20.0, 23.0], || poll(&mut app, &ctx, Ok(RESPONSE.into())));
    assert_eq!(
        (
            state(&app),
            app.gpu_plan.needs_recompute_for(app.state_count()),
            completed_duration(&app),
            remaining
        ),
        (expected, false, Duration::from_secs(3), 0)
    );
}

#[test]
fn running_override_acquires_start_before_matching_completion_reads_end() {
    let (mut app, ctx) = app();
    app.external_gpu_acceptance = Acceptance::Awaiting(AcceptedRun {
        id: RUN_ID,
        expected: DisplayExpectation::None,
    });
    super::super::super::test_hooks::inject_external_gpu_status(ExternalGpuStatus::Running);
    let (_, remaining) = with_times(&[5.0, 12.0], || {
        poll(&mut app, &ctx, Ok("unparsed".into()));
    });
    assert_eq!(
        (
            completed_duration(&app),
            app.external_gpu_started_at,
            remaining
        ),
        (Duration::from_secs(7), None, 0)
    );
}

#[test]
fn parse_failure_without_start_reads_fallback_start_then_end() {
    let (mut app, ctx) = app();
    pending(&mut app);
    app.external_gpu_started_at = None;
    let (_, remaining) = with_times(&[5.0, 12.0], || {
        poll(&mut app, &ctx, Ok("bad".into()));
    });
    assert_eq!(
        (state(&app).status, app.external_gpu_started_at, remaining),
        (
            StatusState::Failed(GpuFailure::Other("Amplitude result missing".into())),
            None,
            0
        )
    );
}

fn completed_duration(app: &QniApp) -> Duration {
    match app.external_gpu_status {
        ExternalGpuStatus::Completed { duration } => duration,
        _ => panic!("expected Completed"),
    }
}

fn completion_duration(started_at: Option<f64>, times: &[f64]) -> (Duration, Option<f64>, usize) {
    let (mut app, ctx) = app();
    app.external_gpu_status = ExternalGpuStatus::Running;
    app.external_gpu_started_at = started_at;
    app.external_gpu_acceptance = Acceptance::Awaiting(AcceptedRun {
        id: RUN_ID,
        expected: DisplayExpectation::None,
    });
    let (_, remaining) = with_times(times, || poll(&mut app, &ctx, Ok("unparsed".into())));
    (
        completed_duration(&app),
        app.external_gpu_started_at,
        remaining,
    )
}

#[test]
fn completion_uses_saved_start_and_one_current_timestamp() {
    assert_eq!(
        completion_duration(Some(5.0), &[12.0]),
        (Duration::from_secs(7), None, 0)
    );
}

#[test]
fn completion_without_start_acquires_start_then_end_timestamp() {
    assert_eq!(
        completion_duration(None, &[5.0, 12.0]),
        (Duration::from_secs(7), None, 0)
    );
}

#[test]
fn completion_clamps_negative_elapsed_time_to_zero() {
    assert_eq!(
        completion_duration(Some(12.0), &[5.0]),
        (Duration::ZERO, None, 0)
    );
}

#[test]
fn transport_failure_clears_start_without_reading_clock() {
    let (mut app, ctx) = app();
    seeded(&mut app);
    let mut expected = state(&app);
    expected.status = StatusState::Failed(GpuFailure::Http(503));
    expected.started_at = None;
    expected.slots = Default::default();
    expected.run_id = None;
    let (_, remaining) = with_times(&[], || poll(&mut app, &ctx, Err(GpuFailure::Http(503))));
    assert_eq!((state(&app), remaining), (expected, 0));
}

#[test]
fn parse_failure_consumes_duration_timestamp_without_exposing_duration() {
    let (mut app, ctx) = app();
    pending(&mut app);
    let (_, remaining) = with_times(&[9.0], || poll(&mut app, &ctx, Ok("bad".into())));
    assert_eq!(
        (state(&app).status, app.external_gpu_started_at, remaining),
        (
            StatusState::Failed(GpuFailure::Other("Amplitude result missing".into())),
            None,
            0
        )
    );
}

fn refresh_case(
    target_ready: bool,
    recompute: bool,
    mode: ExecMode,
    gates: Vec<PlacedGate>,
) -> (bool, bool, bool) {
    let (mut app, ctx) = app();
    app.exec_mode = mode;
    app.placed_gates = gates;
    app.external_gpu_state_refresh_pending = true;
    let result = app.test_process_gpu_recompute(
        target_ready.then_some(eframe::wgpu::TextureFormat::Rgba8Unorm),
        recompute,
        app.state_count(),
        &ctx,
    );
    (
        result,
        app.external_gpu_state_refresh_pending,
        app.gpu_plan.capacity_error().is_some(),
    )
}

#[test]
fn refresh_is_consumed_after_successful_gpu_plan_build() {
    assert_eq!(
        refresh_case(true, true, ExecMode::Gpu, vec![]),
        (true, false, false)
    );
}

#[test]
fn refresh_is_not_consumed_without_gpu_target() {
    assert_eq!(
        refresh_case(false, true, ExecMode::Gpu, vec![]),
        (false, true, false)
    );
}

#[test]
fn refresh_is_not_consumed_without_recompute() {
    assert_eq!(
        refresh_case(true, false, ExecMode::Gpu, vec![]),
        (false, true, false)
    );
}

#[test]
fn refresh_is_not_consumed_by_local_mode_plan_build() {
    assert_eq!(
        refresh_case(true, true, ExecMode::Local, vec![]),
        (true, true, false)
    );
}

#[test]
fn refresh_is_not_consumed_when_local_capacity_is_unavailable() {
    assert_eq!(
        refresh_case(true, true, ExecMode::Gpu, vec![gate(GateKind::H, 1, 0, 31)]),
        (false, true, false)
    );
}

#[test]
fn refresh_is_not_consumed_on_snapshot_capacity_failure() {
    assert_eq!(
        refresh_case(
            true,
            true,
            ExecMode::Gpu,
            vec![gate(GateKind::H, 1, MAX_STEP_SNAPSHOT_SLOTS, 0)]
        ),
        (false, true, true)
    );
}

#[test]
fn refresh_is_not_consumed_on_display_capacity_failure() {
    let gates = (0..=MAX_BLOCH_SLOTS)
        .map(|i| gate(GateKind::BlochDisplay, i as u32 + 1, i, 0))
        .collect();
    assert_eq!(
        refresh_case(true, true, ExecMode::Gpu, gates),
        (false, true, true)
    );
}
