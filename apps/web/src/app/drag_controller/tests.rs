use super::*;
use crate::app::circuit_history::CircuitRevision;
use crate::app::circuit_library;
use crate::app::{CircuitColumnIndex, LiveDragSnap, WireIndex};
use crate::constants::{GATE_SIZE, LINE_Y};

fn circuit_json(app: &QniApp) -> String {
    crate::url_circuit::circuit_to_json(
        &app.placed_gates,
        &app.circuit_blocks,
        &app.circuit_title,
        crate::qubit_count::QubitCount::try_new(app.required_visible_wire_count()).unwrap(),
    )
}

fn layout(gates: &[crate::app::PlacedGate]) -> Vec<(crate::app::GateId, usize, usize, egui::Pos2)> {
    gates
        .iter()
        .map(|gate| {
            (
                gate.id,
                gate.column.as_usize(),
                gate.wire.as_usize(),
                gate.pos,
            )
        })
        .collect()
}

fn inserted_copy(json: &str, wire: usize) -> (QniApp, egui::Context, CircuitInputGeometry) {
    let (mut app, ctx, geometry) = fixture(json);
    start(&mut app, &ctx, &geometry, true);
    let target = egui::pos2(
        (geometry.metrics.slot_centers[0] + geometry.metrics.slot_centers[1]) / 2.0,
        geometry.metrics.line_ys[wire],
    );
    DragController::update_gate_drag_preview(&mut app, pointer(target, false), &geometry.metrics);
    (app, ctx, geometry)
}

#[test]
fn leaving_insert_requests_recompute_even_when_state_count_is_unchanged() {
    let (mut app, _, geometry) = inserted_copy(r#"{"cols":[["H"],["X"]]}"#, 0);
    app.gpu_plan.mark_clean_for(2);
    DragController::update_gate_drag_preview(
        &mut app,
        pointer(egui::pos2(1100.0, 470.0), false),
        &geometry.metrics,
    );

    assert!(app.gpu_plan.needs_recompute_for(2));
}

#[test]
fn leaving_insert_shrinks_the_panel_and_dispatch_to_the_visible_circuit() {
    let (mut app, _, geometry) = inserted_copy(r#"{"cols":[["H"],["X"]]}"#, 1);
    let inserted = app.test_state_panel_dimensions().0;
    DragController::update_gate_drag_preview(
        &mut app,
        pointer(egui::pos2(1100.0, 470.0), false),
        &geometry.metrics,
    );

    assert_eq!(
        (
            inserted,
            app.test_state_panel_dimensions().0,
            app.state_qubits().get()
        ),
        (4, 2, 1)
    );
}

#[test]
fn offgrid_recompute_restores_the_original_gpu_operation_plan() {
    let (mut app, ctx, geometry) = fixture(r#"{"cols":[["H"],["X"]]}"#);
    let format = Some(eframe::wgpu::TextureFormat::Rgba8Unorm);
    app.test_process_gpu_recompute(format, true, 2, &ctx);
    let original = format!("{:?}", app.gpu_plan.sim_ops_for_callback(true));
    start(&mut app, &ctx, &geometry, true);
    let target = egui::pos2(
        (geometry.metrics.slot_centers[0] + geometry.metrics.slot_centers[1]) / 2.0,
        geometry.metrics.line_ys[1],
    );
    DragController::update_gate_drag_preview(&mut app, pointer(target, false), &geometry.metrics);
    app.test_process_gpu_recompute(format, true, 4, &ctx);
    DragController::update_gate_drag_preview(
        &mut app,
        pointer(egui::pos2(1100.0, 470.0), false),
        &geometry.metrics,
    );
    let frame = app.test_state_panel_dimensions();
    app.test_process_gpu_recompute(format, frame.1, frame.0, &ctx);

    assert_eq!(
        format!("{:?}", app.gpu_plan.sim_ops_for_callback(true)),
        original
    );
}

#[test]
fn reentering_insert_restores_the_tentative_gpu_operation_plan() {
    let (mut app, ctx, geometry) = inserted_copy(r#"{"cols":[["H"],["X"]]}"#, 1);
    let format = Some(eframe::wgpu::TextureFormat::Rgba8Unorm);
    app.test_process_gpu_recompute(format, true, 4, &ctx);
    let inserted = format!("{:?}", app.gpu_plan.sim_ops_for_callback(true));
    DragController::update_gate_drag_preview(
        &mut app,
        pointer(egui::pos2(1100.0, 470.0), false),
        &geometry.metrics,
    );
    let frame = app.test_state_panel_dimensions();
    app.test_process_gpu_recompute(format, frame.1, frame.0, &ctx);
    let target = egui::pos2(
        (geometry.metrics.slot_centers[0] + geometry.metrics.slot_centers[1]) / 2.0,
        geometry.metrics.line_ys[1],
    );
    DragController::update_gate_drag_preview(&mut app, pointer(target, false), &geometry.metrics);
    let frame = app.test_state_panel_dimensions();
    app.test_process_gpu_recompute(format, frame.1, frame.0, &ctx);

    assert_eq!(
        (
            frame.0,
            format!("{:?}", app.gpu_plan.sim_ops_for_callback(true))
        ),
        (4, inserted)
    );
}

#[test]
fn normal_move_offgrid_excludes_the_source_and_escape_restores_its_gpu_plan() {
    let (mut app, ctx, geometry) = fixture(r#"{"cols":[[1,"H"],["X"]]}"#);
    let format = Some(eframe::wgpu::TextureFormat::Rgba8Unorm);
    app.test_process_gpu_recompute(format, true, 4, &ctx);
    let original = format!("{:?}", app.gpu_plan.sim_ops_for_callback(true));
    let remaining = vec![app.placed_gates[1].clone()];
    let expected = format!(
        "{:?}",
        crate::simulation_plan::linearize_ops(
            &remaining,
            crate::qubit_count::QubitCount::try_new(1).unwrap(),
            2,
        )
    );
    start(&mut app, &ctx, &geometry, false);
    let target = egui::pos2(
        (geometry.metrics.slot_centers[0] + geometry.metrics.slot_centers[1]) / 2.0,
        geometry.metrics.line_ys[2],
    );
    DragController::update_gate_drag_preview(&mut app, pointer(target, false), &geometry.metrics);
    app.test_process_gpu_recompute(format, true, 8, &ctx);
    DragController::update_gate_drag_preview(
        &mut app,
        pointer(egui::pos2(1100.0, 470.0), false),
        &geometry.metrics,
    );
    let offgrid = app.test_state_panel_dimensions();
    app.test_process_gpu_recompute(format, offgrid.1, offgrid.0, &ctx);
    let actual = format!("{:?}", app.gpu_plan.sim_ops_for_callback(true));
    input_frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        }],
        false,
    );
    let restored = app.test_state_panel_dimensions();
    app.test_process_gpu_recompute(format, restored.1, restored.0, &ctx);

    assert_eq!(
        (
            offgrid.0,
            actual,
            restored.0,
            format!("{:?}", app.gpu_plan.sim_ops_for_callback(true))
        ),
        (2, expected, 4, original)
    );
}

fn rendered_gate_bodies(
    app: &QniApp,
    ctx: &egui::Context,
    geometry: &CircuitInputGeometry,
    origin: egui::Pos2,
    scroll_x: f32,
) -> Vec<(egui::Pos2, bool)> {
    let rect = egui::Rect::from_min_size(origin, egui::vec2(1280.0, 800.0));
    ctx.begin_pass(egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1280.0, 800.0),
        )),
        ..Default::default()
    });
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("hover-test"),
    ));
    let colors = app.colors();
    let dragged = app.dragging.map(|drag| drag.id);
    let insert_preview_painted = app.draw_circuit(
        &painter,
        rect,
        &geometry.metrics,
        &colors,
        false,
        dragged,
        scroll_x,
    );
    if let Some(id) = dragged.filter(|_| !insert_preview_painted) {
        app.draw_drag_preview(
            &painter,
            rect.min - egui::vec2(scroll_x, 0.0),
            &colors,
            id,
            false,
        );
    }
    ctx.end_pass()
        .shapes
        .into_iter()
        .filter_map(|clipped| {
            let (center, fill) = match clipped.shape {
                egui::Shape::Rect(shape)
                    if shape.rect.width() == GATE_SIZE && shape.rect.height() == GATE_SIZE =>
                {
                    (shape.rect.center(), shape.fill)
                }
                egui::Shape::Circle(shape) if shape.radius == GATE_SIZE / 2.0 => {
                    (shape.center, shape.fill)
                }
                _ => return None,
            };
            (fill == colors.box_fill || fill == colors.drag_fill)
                .then_some((center, fill == colors.drag_fill))
        })
        .collect()
}

#[test]
fn insert_hover_draws_existing_rows_in_place_and_copy_at_the_boundary() {
    let (app, ctx, geometry) = inserted_copy(r#"{"cols":[["H"],["X","Z"]]}"#, 0);
    let x0 = geometry.metrics.slot_centers[0];
    let x1 = geometry.metrics.slot_centers[1];
    let y0 = geometry.metrics.line_ys[0];
    let y1 = geometry.metrics.line_ys[1];

    assert_eq!(
        rendered_gate_bodies(&app, &ctx, &geometry, egui::Pos2::ZERO, 0.0),
        vec![
            (egui::pos2(x0, y0), false),
            (egui::pos2((x0 + x1) / 2.0, y0), true),
            (egui::pos2(x1, y0), false),
            (egui::pos2(x1, y1), false),
        ]
    );
}

#[test]
fn normal_insert_hover_draws_trailing_columns_without_precompression() {
    let (mut app, ctx, geometry) = fixture(r#"{"cols":[["H"],["X"],["Z"]]}"#);
    start(&mut app, &ctx, &geometry, false);
    let x1 = geometry.metrics.slot_centers[1];
    let x2 = geometry.metrics.slot_centers[2];
    let y = geometry.metrics.line_ys[0];
    DragController::update_gate_drag_preview(
        &mut app,
        pointer(egui::pos2((x1 + x2) / 2.0, y), false),
        &geometry.metrics,
    );

    assert_eq!(
        rendered_gate_bodies(&app, &ctx, &geometry, egui::Pos2::ZERO, 0.0),
        vec![
            (egui::pos2(x1, y), false),
            (egui::pos2((x1 + x2) / 2.0, y), true),
            (egui::pos2(x2, y), false),
        ]
    );
}

fn hover_y_between_h_and_x(shift: bool) -> (QniApp, egui::Context, CircuitInputGeometry) {
    let (mut app, ctx, geometry) = fixture(r#"{"cols":[["H"],["X"],["Y"]]}"#);
    ctx.begin_pass(egui::RawInput {
        modifiers: egui::Modifiers {
            shift,
            ..Default::default()
        },
        ..Default::default()
    });
    let source = app.placed_gates[2].pos + egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0);
    DragController::handle_pointer_start(&mut app, pointer(source, false), &geometry, &ctx);
    let _ = ctx.end_pass();
    let target = egui::pos2(
        (geometry.metrics.slot_centers[0] + geometry.metrics.slot_centers[1]) / 2.0,
        geometry.metrics.line_ys[0],
    );
    DragController::update_gate_drag_preview(&mut app, pointer(target, false), &geometry.metrics);
    (app, ctx, geometry)
}

#[test]
fn normal_y_insert_hover_paints_h_then_y_then_x() {
    let (app, ctx, geometry) = hover_y_between_h_and_x(false);
    let x0 = geometry.metrics.slot_centers[0];
    let x1 = geometry.metrics.slot_centers[1];
    let y = geometry.metrics.line_ys[0];

    assert_eq!(
        rendered_gate_bodies(&app, &ctx, &geometry, egui::Pos2::ZERO, 0.0),
        vec![
            (egui::pos2(x0, y), false),
            (egui::pos2((x0 + x1) / 2.0, y), true),
            (egui::pos2(x1, y), false)
        ]
    );
}

#[test]
fn copied_y_insert_hover_paints_h_then_y_then_right_columns() {
    let (app, ctx, geometry) = hover_y_between_h_and_x(true);
    let x0 = geometry.metrics.slot_centers[0];
    let x1 = geometry.metrics.slot_centers[1];
    let x2 = geometry.metrics.slot_centers[2];
    let y = geometry.metrics.line_ys[0];

    assert_eq!(
        rendered_gate_bodies(&app, &ctx, &geometry, egui::Pos2::ZERO, 0.0),
        vec![
            (egui::pos2(x0, y), false),
            (egui::pos2((x0 + x1) / 2.0, y), true),
            (egui::pos2(x1, y), false),
            (egui::pos2(x2, y), false)
        ]
    );
}

#[test]
fn insert_layer_positions_apply_content_origin_and_scroll_once() {
    let (app, ctx, geometry) = hover_y_between_h_and_x(false);
    let origin = egui::pos2(96.0, 48.0);
    let scroll = 56.0;
    let offset = origin.to_vec2() - egui::vec2(scroll, 0.0);
    let x0 = geometry.metrics.slot_centers[0];
    let x1 = geometry.metrics.slot_centers[1];
    let y = geometry.metrics.line_ys[0];

    assert_eq!(
        rendered_gate_bodies(&app, &ctx, &geometry, origin, scroll),
        vec![
            (egui::pos2(x0, y) + offset, false),
            (egui::pos2((x0 + x1) / 2.0, y) + offset, true),
            (egui::pos2(x1, y) + offset, false)
        ]
    );
}

#[test]
fn floating_and_slot_previews_apply_content_origin_and_scroll_once() {
    let (mut app, ctx, geometry) = hover_y_between_h_and_x(false);
    let origin = egui::pos2(96.0, 48.0);
    let scroll = 56.0;
    let offset = origin.to_vec2() - egui::vec2(scroll, 0.0);
    let floating = egui::pos2(700.0, 470.0);
    DragController::update_gate_drag_preview(&mut app, pointer(floating, false), &geometry.metrics);
    let outside = rendered_gate_bodies(&app, &ctx, &geometry, origin, scroll)
        .last()
        .copied();
    let slot = egui::pos2(
        geometry.metrics.slot_centers[1],
        geometry.metrics.line_ys[1],
    );
    DragController::update_gate_drag_preview(&mut app, pointer(slot, false), &geometry.metrics);
    let snapped = rendered_gate_bodies(&app, &ctx, &geometry, origin, scroll)
        .last()
        .copied();

    assert_eq!(
        (outside, snapped),
        (Some((floating + offset, true)), Some((slot + offset, true)))
    );
}

#[test]
fn gpu_display_passes_bracket_the_ghost_with_identical_viewports() {
    let (mut app, ctx, geometry) = fixture(r#"{"cols":[["Probability"],["Probability"],["Y"]]}"#);
    app.test_process_gpu_recompute(Some(eframe::wgpu::TextureFormat::Rgba8Unorm), true, 2, &ctx);
    ctx.begin_pass(egui::RawInput::default());
    let source = app.placed_gates[2].pos + egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0);
    DragController::handle_pointer_start(&mut app, pointer(source, false), &geometry, &ctx);
    let _ = ctx.end_pass();
    let split = (geometry.metrics.slot_centers[0] + geometry.metrics.slot_centers[1]) / 2.0;
    DragController::update_gate_drag_preview(
        &mut app,
        pointer(egui::pos2(split, geometry.metrics.line_ys[0]), false),
        &geometry.metrics,
    );
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0));
    ctx.begin_pass(egui::RawInput {
        screen_rect: Some(rect),
        ..Default::default()
    });
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("gpu-layer-test"),
    ));
    let colors = app.colors();
    app.draw_circuit(
        &painter,
        rect,
        &geometry.metrics,
        &colors,
        true,
        app.dragging.map(|d| d.id),
        0.0,
    );
    let order = ctx
        .end_pass()
        .shapes
        .into_iter()
        .filter_map(|clipped| match clipped.shape {
            egui::Shape::Callback(cb) => Some((
                if clipped.clip_rect.max.x <= split {
                    "left"
                } else {
                    "right"
                },
                cb.rect,
            )),
            egui::Shape::Rect(shape) if shape.fill == colors.drag_fill => Some(("ghost", rect)),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        order,
        vec![("left", rect), ("ghost", rect), ("right", rect)]
    );
}

fn side_copy(
    json: &str,
    source_index: usize,
    x_offset: f32,
) -> (
    QniApp,
    egui::Context,
    CircuitInputGeometry,
    egui::Pos2,
    crate::app::GateId,
) {
    let (mut app, ctx, geometry) = fixture(json);
    let source = app.placed_gates[source_index].clone();
    let rect = crate::layout::gate_visible_rect(&source, source.pos);
    let pos = rect.center() + egui::vec2(x_offset, 0.0);
    ctx.begin_pass(egui::RawInput {
        modifiers: egui::Modifiers {
            shift: true,
            ..Default::default()
        },
        ..Default::default()
    });
    DragController::handle_pointer_start(&mut app, pointer(pos, false), &geometry, &ctx);
    let _ = ctx.end_pass();
    (app, ctx, geometry, pos, source.id)
}

#[test]
fn shift_left_click_preview_snapshot() {
    let (app, ctx, geometry, _, _) = side_copy(r#"{"cols":[["X"],["H"],["Z"]]}"#, 1, -10.0);
    insta::assert_debug_snapshot!(rendered_gate_bodies(
        &app,
        &ctx,
        &geometry,
        egui::Pos2::ZERO,
        0.0
    ));
}

#[test]
fn shift_right_click_preview_snapshot() {
    let (app, ctx, geometry, _, _) = side_copy(r#"{"cols":[["X"],["H"],["Z"]]}"#, 1, 10.0);
    insta::assert_debug_snapshot!(rendered_gate_bodies(
        &app,
        &ctx,
        &geometry,
        egui::Pos2::ZERO,
        0.0
    ));
}

#[test]
fn shift_left_click_inserts_before_source_and_after_occupied_left_neighbor() {
    let (mut app, ctx, geometry, pos, source) =
        side_copy(r#"{"cols":[["X"],["H"],["Z"]]}"#, 1, -10.0);
    drop_at(&mut app, &ctx, &geometry, pos);

    assert_eq!(
        (
            circuit_json(&app),
            app.placed_gates
                .iter()
                .find(|g| g.id == source)
                .unwrap()
                .column
                .as_usize()
        ),
        (r#"{"cols":[["X"],["H"],["H"],["Z"]]}"#.to_owned(), 2)
    );
}

#[test]
fn shift_right_click_inserts_after_source_and_before_occupied_right_neighbor() {
    let (mut app, ctx, geometry, pos, source) =
        side_copy(r#"{"cols":[["X"],["H"],["Z"]]}"#, 1, 10.0);
    drop_at(&mut app, &ctx, &geometry, pos);

    assert_eq!(
        (
            circuit_json(&app),
            app.placed_gates
                .iter()
                .find(|g| g.id == source)
                .unwrap()
                .column
                .as_usize()
        ),
        (r#"{"cols":[["X"],["H"],["H"],["Z"]]}"#.to_owned(), 1)
    );
}

#[test]
fn shift_center_click_uses_the_right_half() {
    let (mut app, ctx, geometry, pos, source) = side_copy(r#"{"cols":[["H"],["X"]]}"#, 0, 0.0);
    drop_at(&mut app, &ctx, &geometry, pos);

    assert_eq!(
        (
            circuit_json(&app),
            app.placed_gates
                .iter()
                .find(|g| g.id == source)
                .unwrap()
                .column
                .as_usize()
        ),
        (r#"{"cols":[["H"],["H"],["X"]]}"#.to_owned(), 0)
    );
}

#[test]
fn shift_left_click_at_column_zero_keeps_source_to_the_right() {
    let (mut app, ctx, geometry, pos, source) = side_copy(r#"{"cols":[["H"],["X"]]}"#, 0, -10.0);
    drop_at(&mut app, &ctx, &geometry, pos);

    assert_eq!(
        (
            circuit_json(&app),
            app.placed_gates
                .iter()
                .find(|g| g.id == source)
                .unwrap()
                .column
                .as_usize()
        ),
        (r#"{"cols":[["H"],["H"],["X"]]}"#.to_owned(), 1)
    );
}

#[test]
fn shift_left_click_inserts_between_columns_even_when_neighbor_cell_is_free() {
    let (mut app, ctx, geometry, pos, source) =
        side_copy(r#"{"cols":[["X"],[1,"Z"],["H"]]}"#, 2, -10.0);
    drop_at(&mut app, &ctx, &geometry, pos);

    assert_eq!(
        (
            circuit_json(&app),
            app.placed_gates
                .iter()
                .find(|g| g.id == source)
                .unwrap()
                .column
                .as_usize()
        ),
        (r#"{"cols":[["X"],[1,"Z"],["H"],["H"]]}"#.to_owned(), 3)
    );
}

#[test]
fn shift_right_click_inserts_between_columns_even_when_neighbor_cell_is_free() {
    let (mut app, ctx, geometry, pos, source) =
        side_copy(r#"{"cols":[["H"],[1,"Z"],["X"]]}"#, 0, 10.0);
    drop_at(&mut app, &ctx, &geometry, pos);

    assert_eq!(
        (
            circuit_json(&app),
            app.placed_gates
                .iter()
                .find(|g| g.id == source)
                .unwrap()
                .column
                .as_usize()
        ),
        (r#"{"cols":[["H"],["H"],[1,"Z"],["X"]]}"#.to_owned(), 0)
    );
}

#[test]
fn side_click_copy_preserves_rotation_angle() {
    let (mut app, ctx, geometry, pos, _) = side_copy(r#"{"cols":[["Rx(π/3)"],["X"]]}"#, 0, 10.0);
    drop_at(&mut app, &ctx, &geometry, pos);

    assert_eq!(
        circuit_json(&app),
        r#"{"cols":[["Rx(π_3)"],["Rx(π_3)"],["X"]]}"#
    );
}

#[test]
fn side_click_copy_preserves_span_and_checks_all_its_wires() {
    let (mut app, ctx, geometry, pos, _) = side_copy(r#"{"cols":[["QFT3"],[1,1,"X"]]}"#, 0, 10.0);
    drop_at(&mut app, &ctx, &geometry, pos);

    assert_eq!(
        circuit_json(&app),
        r#"{"cols":[["QFT3"],["QFT3"],[1,1,"X"]]}"#
    );
}

#[test]
fn side_click_copy_is_one_undoable_edit() {
    let json = r#"{"cols":[["H"],["X"]]}"#;
    let (mut app, ctx, geometry, pos, _) = side_copy(json, 0, 10.0);
    drop_at(&mut app, &ctx, &geometry, pos);
    let copied = circuit_json(&app);
    app.undo_circuit(&ctx);

    assert_eq!(
        (copied, circuit_json(&app), app.can_undo_circuit()),
        (
            r#"{"cols":[["H"],["H"],["X"]]}"#.to_owned(),
            json.to_owned(),
            false
        )
    );
}

#[test]
fn gpu_insertion_plan_projects_all_rows_at_the_final_positions() {
    let (mut app, ctx, geometry) = fixture(r#"{"cols":[["H"],["X","Z"]]}"#);
    start(&mut app, &ctx, &geometry, true);
    let pos = egui::pos2(
        (geometry.metrics.slot_centers[0] + geometry.metrics.slot_centers[1]) / 2.0,
        LINE_Y,
    );
    DragController::update_gate_drag_preview(&mut app, pointer(pos, false), &geometry.metrics);
    let raw = layout(&app.placed_gates);
    let preview = layout(app.gpu_plan_gates().as_ref());
    let unchanged = layout(&app.placed_gates) == raw;
    drop_at(&mut app, &ctx, &geometry, pos);

    assert_eq!((preview, unchanged), (layout(&app.placed_gates), true));
}

#[test]
fn gpu_insertion_plan_matches_normal_move_source_column_removal() {
    let (mut app, ctx, geometry) = fixture(r#"{"cols":[["H"],["X"],["Z"]]}"#);
    start(&mut app, &ctx, &geometry, false);
    let pos = egui::pos2(
        (geometry.metrics.slot_centers[1] + geometry.metrics.slot_centers[2]) / 2.0,
        LINE_Y,
    );
    DragController::update_gate_drag_preview(&mut app, pointer(pos, false), &geometry.metrics);
    let preview = layout(app.gpu_plan_gates().as_ref());
    drop_at(&mut app, &ctx, &geometry, pos);

    assert_eq!(preview, layout(&app.placed_gates));
}

#[test]
fn gpu_insertion_plan_matches_wide_gate_shift_and_compaction() {
    let (mut app, ctx, geometry) = fixture(r#"{"cols":[["Amps3"],[],["X"],["Z"]]}"#);
    start(&mut app, &ctx, &geometry, true);
    let pos = egui::pos2(
        (geometry.metrics.slot_centers[2] + geometry.metrics.slot_centers[3]) / 2.0,
        LINE_Y,
    );
    DragController::update_gate_drag_preview(&mut app, pointer(pos, false), &geometry.metrics);
    let preview = layout(app.gpu_plan_gates().as_ref());
    drop_at(&mut app, &ctx, &geometry, pos);

    assert_eq!(preview, layout(&app.placed_gates));
}

#[test]
fn sparse_insert_target_and_scroll_stay_stable_across_held_frames() {
    let json = format!(r#"{{"cols":[["H"],{}["X"]]}}"#, "[],".repeat(29));
    let (mut app, ctx, _) = fixture(&json);
    let origin = app.placed_gates[0].pos + egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0);
    input_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(origin),
            egui::Event::PointerButton {
                pos: origin,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers {
                    shift: true,
                    ..Default::default()
                },
            },
        ],
        true,
    );
    app.circuit_scroll_x = 650.0;
    let target = egui::pos2(
        app.placed_gates[1].pos.x + GATE_SIZE / 2.0 + crate::constants::SLOT_SPACING / 2.0 - 650.0,
        LINE_Y,
    );
    input_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(target)],
        true,
    );
    for _ in 0..4 {
        let _ = app.gpu_plan_gates();
        input_frame(&mut app, &ctx, vec![], true);
    }

    assert_eq!(
        (app.dragging_live_snap, app.circuit_scroll_x),
        (
            Some(LiveDragSnap::Insert {
                column: CircuitColumnIndex::new(31),
                wire: WireIndex::ZERO
            }),
            650.0
        )
    );
}

#[test]
fn moving_out_of_insert_restores_trailing_gate_positions() {
    let (mut app, ctx, geometry) = fixture(r#"{"cols":[["H"],["X","Z"]]}"#);
    let trailing = layout(&app.placed_gates)[1..3].to_vec();
    start(&mut app, &ctx, &geometry, true);
    DragController::update_gate_drag_preview(
        &mut app,
        pointer(
            egui::pos2(
                (geometry.metrics.slot_centers[0] + geometry.metrics.slot_centers[1]) / 2.0,
                LINE_Y,
            ),
            false,
        ),
        &geometry.metrics,
    );
    let _ = app.gpu_plan_gates();
    DragController::update_gate_drag_preview(
        &mut app,
        pointer(
            egui::pos2(
                geometry.metrics.slot_centers[2],
                geometry.metrics.line_ys[1],
            ),
            false,
        ),
        &geometry.metrics,
    );
    let slots = layout(app.gpu_plan_gates().as_ref());
    DragController::update_gate_drag_preview(
        &mut app,
        pointer(egui::pos2(1100.0, 470.0), false),
        &geometry.metrics,
    );
    let off_grid = layout(app.gpu_plan_gates().as_ref());

    assert_eq!(
        (
            slots[1..3].to_vec(),
            off_grid[1..3].to_vec(),
            app.dragging_live_snap
        ),
        (trailing.clone(), trailing, None)
    );
}

#[test]
fn escape_from_insert_restores_layout_and_preserves_previous_undo() {
    let json = r#"{"cols":[["H"],["X"]]}"#;
    let previous = r#"{"cols":[["Z"]]}"#;
    let (mut app, ctx, geometry) = fixture(json);
    let original = layout(&app.placed_gates);
    app.circuit_revision = CircuitRevision::starting_at(previous.to_owned());
    app.circuit_revision.commit(json.to_owned());
    let origin = app.placed_gates[0].pos + egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0);
    input_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(origin),
            egui::Event::PointerButton {
                pos: origin,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers {
                    shift: true,
                    ..Default::default()
                },
            },
        ],
        true,
    );
    let target = egui::pos2(
        (geometry.metrics.slot_centers[0] + geometry.metrics.slot_centers[1]) / 2.0,
        LINE_Y,
    );
    input_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(target)],
        true,
    );
    let _ = app.gpu_plan_gates();
    input_frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        }],
        false,
    );
    input_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerButton {
            pos: target,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::default(),
        }],
        false,
    );
    let cancelled = (circuit_json(&app), layout(app.gpu_plan_gates().as_ref()));
    app.undo_circuit(&ctx);

    assert_eq!(
        (cancelled, circuit_json(&app)),
        ((json.to_owned(), original), previous.to_owned())
    );
}

fn fixture(json: &str) -> (QniApp, egui::Context, CircuitInputGeometry) {
    let ctx = egui::Context::default();
    let mut app = QniApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()));
    app.load_circuit_json_into_editor(json, &ctx);
    app.library = circuit_library::for_startup(json.to_owned(), true).0;
    app.circuit_revision = CircuitRevision::starting_at(json.to_owned());
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0));
    let geometry = CircuitInputGeometry::new(rect, rect, 3, 6, &app.palette);
    (app, ctx, geometry)
}

fn pointer(pos: egui::Pos2, released: bool) -> DragPointer {
    DragPointer {
        screen_pos: Some(pos),
        local_pos: Some(pos),
        down: !released,
        start: !released,
        released,
        shift_at_start: None,
    }
}

fn start(app: &mut QniApp, ctx: &egui::Context, geometry: &CircuitInputGeometry, shift: bool) {
    ctx.begin_pass(egui::RawInput {
        modifiers: egui::Modifiers {
            shift,
            ..Default::default()
        },
        ..Default::default()
    });
    let pos = app.placed_gates[0].pos + egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0);
    DragController::handle_pointer_start(app, pointer(pos, false), geometry, ctx);
    let _ = ctx.end_pass();
}

fn drop_at(
    app: &mut QniApp,
    ctx: &egui::Context,
    geometry: &CircuitInputGeometry,
    pos: egui::Pos2,
) {
    let input = pointer(pos, true);
    DragController::update_gate_drag_preview(app, input, &geometry.metrics);
    DragController::commit_gate_drop(app, input, &geometry.metrics, ctx);
}

fn input_frame(app: &mut QniApp, ctx: &egui::Context, events: Vec<egui::Event>, shift: bool) {
    ctx.begin_pass(egui::RawInput {
        events,
        modifiers: egui::Modifiers {
            shift,
            ..Default::default()
        },
        ..Default::default()
    });
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0));
    app.handle_input(rect, ctx, rect, false);
    let _ = ctx.end_pass();
}

#[test]
fn shift_pickup_preserves_multi_wire_single_gate() {
    let (mut app, ctx, geometry) = fixture(r#"{"cols":[["QFT3"]]}"#);
    start(&mut app, &ctx, &geometry, true);

    assert_eq!(
        app.placed_gates
            .iter()
            .map(|gate| gate.span.get())
            .collect::<Vec<_>>(),
        vec![3, 3]
    );
}

#[test]
fn ordinary_drop_moves_without_copying() {
    let (mut app, ctx, geometry) = fixture(r#"{"cols":[["H"],["X"]]}"#);
    start(&mut app, &ctx, &geometry, false);
    drop_at(
        &mut app,
        &ctx,
        &geometry,
        egui::pos2(
            geometry.metrics.slot_centers[0],
            geometry.metrics.line_ys[1],
        ),
    );

    assert_eq!(circuit_json(&app), r#"{"cols":[[1,"H"],["X"]]}"#);
}

#[test]
fn shift_pickup_preserves_source_and_parametric_angle() {
    let (mut app, ctx, geometry) = fixture(r#"{"cols":[["Rx(π/3)"]]}"#);
    let source = app.placed_gates[0].clone();
    start(&mut app, &ctx, &geometry, true);
    let drag = app.dragging.unwrap();
    let copy = app
        .placed_gates
        .iter()
        .find(|gate| gate.id == drag.id)
        .unwrap();

    assert_eq!(
        (
            app.placed_gates.len(),
            drag.id != source.id,
            drag.original_column,
            copy.kind,
            copy.span,
            copy.angle,
            app.placed_gates[0].pos
        ),
        (
            2,
            true,
            None,
            source.kind,
            source.span,
            source.angle,
            source.pos
        )
    );
}

#[test]
fn ordinary_pickup_moves_the_original_gate() {
    let (mut app, ctx, geometry) = fixture(r#"{"cols":[["H"]]}"#);
    let id = app.placed_gates[0].id;
    start(&mut app, &ctx, &geometry, false);
    let drag = app.dragging.unwrap();

    assert_eq!(
        (app.placed_gates.len(), drag.id, drag.original_column),
        (1, id, Some(CircuitColumnIndex::ZERO))
    );
}

#[test]
fn releasing_shift_before_drop_keeps_the_original() {
    let (mut app, ctx, geometry) = fixture(r#"{"cols":[["H"],["X"]]}"#);
    let source_id = app.placed_gates[0].id;
    start(&mut app, &ctx, &geometry, true);
    ctx.begin_pass(egui::RawInput::default());
    drop_at(
        &mut app,
        &ctx,
        &geometry,
        egui::pos2(
            geometry.metrics.slot_centers[1],
            geometry.metrics.line_ys[1],
        ),
    );
    let _ = ctx.end_pass();

    assert_eq!(
        (
            circuit_json(&app),
            app.placed_gates
                .iter()
                .find(|gate| gate.id == source_id)
                .map(|gate| (gate.column, gate.wire))
        ),
        (
            r#"{"cols":[["H"],["X","H"]]}"#.to_owned(),
            Some((CircuitColumnIndex::ZERO, WireIndex::ZERO))
        )
    );
}

#[test]
fn copy_insert_preserves_source_and_shifts_the_whole_column() {
    let (mut app, ctx, geometry) = fixture(r#"{"cols":[["H"],["X","Z"]]}"#);
    start(&mut app, &ctx, &geometry, true);
    drop_at(
        &mut app,
        &ctx,
        &geometry,
        egui::pos2(
            (geometry.metrics.slot_centers[0] + geometry.metrics.slot_centers[1]) / 2.0,
            LINE_Y,
        ),
    );

    assert_eq!(circuit_json(&app), r#"{"cols":[["H"],["H"],["X","Z"]]}"#);
}

#[test]
fn copy_insert_preview_targets_the_same_column_as_drop() {
    let (mut app, ctx, geometry) = fixture(r#"{"cols":[["H"],["X"]]}"#);
    start(&mut app, &ctx, &geometry, true);
    DragController::update_gate_drag_preview(
        &mut app,
        pointer(
            egui::pos2(
                (geometry.metrics.slot_centers[0] + geometry.metrics.slot_centers[1]) / 2.0,
                LINE_Y,
            ),
            false,
        ),
        &geometry.metrics,
    );

    assert_eq!(
        app.dragging_live_snap,
        Some(LiveDragSnap::Insert {
            column: CircuitColumnIndex::new(1),
            wire: WireIndex::ZERO
        })
    );
}

#[test]
fn completed_copy_is_one_undoable_edit() {
    let json = r#"{"cols":[["H"],["X"]]}"#;
    let (mut app, ctx, geometry) = fixture(json);
    start(&mut app, &ctx, &geometry, true);
    drop_at(
        &mut app,
        &ctx,
        &geometry,
        egui::pos2(
            geometry.metrics.slot_centers[1],
            geometry.metrics.line_ys[1],
        ),
    );
    let copied = (circuit_json(&app), app.can_undo_circuit());
    app.undo_circuit(&ctx);

    assert_eq!(
        (copied, circuit_json(&app), app.can_undo_circuit()),
        (
            (r#"{"cols":[["H"],["X","H"]]}"#.to_owned(), true),
            json.to_owned(),
            false
        )
    );
}

#[test]
fn escape_cancels_copy_without_consuming_previous_undo() {
    let json = r#"{"cols":[["H"]]}"#;
    let (mut app, ctx, geometry) = fixture(json);
    let previous = r#"{"cols":[["X"]]}"#;
    app.circuit_revision = CircuitRevision::starting_at(previous.to_owned());
    app.circuit_revision.commit(json.to_owned());
    let origin = app.placed_gates[0].pos + egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0);
    input_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(origin),
            egui::Event::PointerButton {
                pos: origin,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers {
                    shift: true,
                    ..Default::default()
                },
            },
        ],
        true,
    );
    let target = egui::pos2(
        geometry.metrics.slot_centers[1],
        geometry.metrics.line_ys[1],
    );
    input_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(target)],
        false,
    );
    input_frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        }],
        false,
    );
    input_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerButton {
            pos: target,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::default(),
        }],
        false,
    );
    let cancelled = circuit_json(&app);
    app.undo_circuit(&ctx);

    assert_eq!(
        (cancelled, circuit_json(&app), app.dragging.is_none()),
        (json.to_owned(), previous.to_owned(), true)
    );
}

#[test]
fn shift_click_tolerates_jitter_at_click_distance() {
    let (mut app, ctx, geometry, pos, _) = side_copy(r#"{"cols":[["H"],["X"]]}"#, 0, 10.0);
    let distance = app.dragging.unwrap().click_copy.unwrap().max_distance;
    drop_at(&mut app, &ctx, &geometry, pos + egui::vec2(distance, 0.0));
    assert_eq!(circuit_json(&app), r#"{"cols":[["H"],["H"],["X"]]}"#);
}
#[test]
fn shift_drag_returning_to_pickup_does_not_restore_click_target() {
    let (mut app, ctx, geometry, pos, source) = side_copy(r#"{"cols":[["H"],["X"]]}"#, 0, 10.0);
    let distance = app.dragging.unwrap().click_copy.unwrap().max_distance;
    DragController::update_gate_drag_preview(
        &mut app,
        pointer(pos + egui::vec2(distance + 1.0, 0.0), false),
        &geometry.metrics,
    );
    drop_at(&mut app, &ctx, &geometry, pos);
    assert_eq!(
        (
            circuit_json(&app),
            app.placed_gates
                .iter()
                .find(|g| g.id == source)
                .unwrap()
                .column
                .as_usize()
        ),
        (r#"{"cols":[["H"],["H"],["X"]]}"#.to_owned(), 1)
    );
}

fn primary(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers {
            shift: true,
            ..Default::default()
        },
    }
}
#[test]
fn same_frame_return_after_threshold_remains_a_drag() {
    let (mut app, ctx, _) = fixture(r#"{"cols":[["H"],["X"]]}"#);
    let pos = app.placed_gates[0].pos + egui::vec2(GATE_SIZE / 2.0 + 10.0, GATE_SIZE / 2.0);
    input_frame(&mut app, &ctx, vec![primary(pos, true)], true);
    input_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(pos + egui::vec2(20.0, 0.0)),
            egui::Event::PointerMoved(pos),
        ],
        true,
    );
    assert!(app.dragging.unwrap().click_copy.is_none());
}
#[test]
fn old_release_followed_by_new_press_does_not_commit_held_copy() {
    let json = r#"{"cols":[["H"],["X"]]}"#;
    let (mut app, ctx, _) = fixture(json);
    let empty = egui::pos2(1000.0, 470.0);
    let pos = app.placed_gates[0].pos + egui::vec2(GATE_SIZE / 2.0 + 10.0, GATE_SIZE / 2.0);
    input_frame(&mut app, &ctx, vec![primary(empty, true)], true);
    input_frame(
        &mut app,
        &ctx,
        vec![
            primary(empty, false),
            primary(pos, true),
            egui::Event::PointerMoved(pos),
        ],
        true,
    );
    assert_eq!(
        (
            app.dragging.is_some(),
            app.library.active().circuit_json.clone()
        ),
        (true, json.to_owned())
    );
}

#[test]
fn pickup_frame_return_after_threshold_remains_a_drag() {
    let (mut app, ctx, _) = fixture(r#"{"cols":[["H"],["X"]]}"#);
    let pos = app.placed_gates[0].pos + egui::vec2(GATE_SIZE / 2.0 + 10.0, GATE_SIZE / 2.0);
    input_frame(
        &mut app,
        &ctx,
        vec![
            primary(pos, true),
            egui::Event::PointerMoved(pos + egui::vec2(20.0, 0.0)),
            egui::Event::PointerMoved(pos),
        ],
        true,
    );
    assert!(app.dragging.unwrap().click_copy.is_none());
}

#[test]
fn shift_at_press_is_latched_when_released_in_pickup_frame() {
    let (mut app, ctx, _) = fixture(r#"{"cols":[["H"],["X"]]}"#);
    let pos = app.placed_gates[0].pos + egui::vec2(GATE_SIZE / 2.0 + 10.0, GATE_SIZE / 2.0);
    input_frame(&mut app, &ctx, vec![primary(pos, true)], false);
    assert_eq!(app.placed_gates.len(), 3);
}

#[test]
fn wide_gate_right_copy_starts_after_its_entire_footprint() {
    let (mut app, ctx, geometry, pos, _) = side_copy(r#"{"cols":[["Amps3"],[],["X"]]}"#, 0, 10.0);
    drop_at(&mut app, &ctx, &geometry, pos);
    assert_eq!(
        circuit_json(&app),
        r#"{"cols":[["Amps3"],[1],["Amps3"],[1],["X"]]}"#
    );
}

#[test]
fn single_gate_right_click_previews_between_source_and_empty_next_column() {
    let (app, _, geometry, _, _) = side_copy(r#"{"cols":[["H"]]}"#, 0, 10.0);
    let copy = app
        .placed_gates
        .iter()
        .find(|g| g.id == app.dragging.unwrap().id)
        .unwrap();
    assert_eq!(
        (app.dragging_live_snap, copy.pos.x + GATE_SIZE / 2.0),
        (
            Some(LiveDragSnap::Insert {
                column: CircuitColumnIndex::new(1),
                wire: WireIndex::ZERO
            }),
            (geometry.metrics.slot_centers[0] + geometry.metrics.slot_centers[1]) / 2.0
        )
    );
}
#[test]
fn free_left_neighbor_still_previews_at_source_left_boundary() {
    let (app, _, geometry, _, _) = side_copy(r#"{"cols":[["X"],[1,"Z"],["H"]]}"#, 2, -10.0);
    let copy = app
        .placed_gates
        .iter()
        .find(|g| g.id == app.dragging.unwrap().id)
        .unwrap();
    assert_eq!(
        (app.dragging_live_snap, copy.pos.x + GATE_SIZE / 2.0),
        (
            Some(LiveDragSnap::Insert {
                column: CircuitColumnIndex::new(2),
                wire: WireIndex::ZERO
            }),
            (geometry.metrics.slot_centers[1] + geometry.metrics.slot_centers[2]) / 2.0
        )
    );
}

fn off_circuit() -> egui::Pos2 {
    egui::pos2(1100.0, 470.0)
}

#[test]
fn removing_a_gate_before_a_block_moves_the_block_left() {
    let (mut app, ctx, geometry) = fixture(r#"{"cols":[["X"],["{a"],["H"],["Z"],["}"]]}"#);
    start(&mut app, &ctx, &geometry, false);
    drop_at(&mut app, &ctx, &geometry, off_circuit());

    assert_eq!(circuit_json(&app), r#"{"cols":[["{a"],["H"],["Z"],["}"]]}"#);
}

#[test]
fn removing_the_last_gate_of_a_block_drops_the_block() {
    let (mut app, ctx, geometry) = fixture(r#"{"cols":[["{a"],["H"],["}"],["X"]]}"#);
    start(&mut app, &ctx, &geometry, false);
    drop_at(&mut app, &ctx, &geometry, off_circuit());

    assert_eq!(circuit_json(&app), r#"{"cols":[["X"]]}"#);
}

#[test]
fn inserting_between_block_columns_widens_the_block() {
    let (mut app, ctx, geometry) = fixture(r#"{"cols":[["X"],["{a"],["H"],["Z"],["}"]]}"#);
    start(&mut app, &ctx, &geometry, false);
    drop_at(
        &mut app,
        &ctx,
        &geometry,
        egui::pos2(
            (geometry.metrics.slot_centers[1] + geometry.metrics.slot_centers[2]) / 2.0,
            geometry.metrics.line_ys[0],
        ),
    );

    assert_eq!(
        circuit_json(&app),
        r#"{"cols":[["{a"],["H"],["X"],["Z"],["}"]]}"#
    );
}

#[test]
fn moving_a_gate_into_a_block_slot_keeps_the_block() {
    let (mut app, ctx, geometry) = fixture(r#"{"cols":[["X"],["{a"],["H"],["}"]]}"#);
    start(&mut app, &ctx, &geometry, false);
    drop_at(
        &mut app,
        &ctx,
        &geometry,
        egui::pos2(
            geometry.metrics.slot_centers[1],
            geometry.metrics.line_ys[1],
        ),
    );

    assert_eq!(circuit_json(&app), r#"{"cols":[["{a"],["H","X"],["}"]]}"#);
}

#[test]
fn undo_restores_a_dropped_block() {
    let (mut app, ctx, geometry) = fixture(r#"{"cols":[["{a"],["H"],["}"],["X"]]}"#);
    start(&mut app, &ctx, &geometry, false);
    drop_at(&mut app, &ctx, &geometry, off_circuit());
    app.undo_circuit(&ctx);

    assert_eq!(circuit_json(&app), r#"{"cols":[["{a"],["H"],["}"],["X"]]}"#);
}

#[test]
fn widening_the_last_gate_of_a_block_widens_the_block() {
    let (mut app, _, _) = fixture(r#"{"cols":[["{a"],["Amps2"],[1],["}"],["H"]]}"#);
    let gate = app.placed_gates[0].id;
    app.shift_trailing_gates_after_width_change(gate, CircuitColumnIndex::ZERO, 2, 4);

    assert_eq!(
        circuit_json(&app),
        r#"{"cols":[["{a"],["Amps2"],[1],[1],[1],["}"],["H"]]}"#
    );
}

#[test]
fn removing_the_only_gate_drops_its_block() {
    let (mut app, ctx, geometry) = fixture(r#"{"cols":[["{a"],["H"],["}"]]}"#);
    start(&mut app, &ctx, &geometry, false);
    drop_at(&mut app, &ctx, &geometry, off_circuit());

    assert_eq!(circuit_json(&app), r#"{"cols":[]}"#);
}

#[test]
fn editing_trims_empty_columns_at_the_end_of_a_block() {
    let (mut app, ctx, geometry) = fixture(r#"{"cols":[["{a"],["H"],[1],["}"]]}"#);
    start(&mut app, &ctx, &geometry, false);
    drop_at(
        &mut app,
        &ctx,
        &geometry,
        egui::pos2(
            geometry.metrics.slot_centers[0],
            geometry.metrics.line_ys[1],
        ),
    );

    assert_eq!(circuit_json(&app), r#"{"cols":[["{a"],[1,"H"],["}"]]}"#);
}
