use eframe::egui;

use crate::app::circuit_history::CircuitRevision;
use crate::app::{circuit_library, CircuitBlockLabelSlot, EmbedStartup, QniApp};
use crate::constants::{PALETTE_ROW_Y, PALETTE_SIZE};
use crate::layout::{layout_metrics, palette_start_x, LayoutMetrics, PaletteItem};
use crate::render::circuit_block_rect;

fn screen() -> egui::Rect {
    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))
}

fn editable(json: &str) -> (QniApp, egui::Context) {
    let ctx = egui::Context::default();
    let mut app = QniApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()));
    app.library = circuit_library::for_startup(json.to_owned(), true).0;
    app.load_circuit_json_into_editor(json, &ctx);
    app.circuit_revision = CircuitRevision::starting_at(json.to_owned());
    (app, ctx)
}

fn embed(json: &str, palette: &[&str]) -> (QniApp, egui::Context) {
    let ctx = egui::Context::default();
    let startup = EmbedStartup::parse(json, true, Some(palette), None).unwrap();
    let app = QniApp::new_with_startup(
        &eframe::CreationContext::_new_kittest(ctx.clone()),
        Some(startup),
    );
    (app, ctx)
}

fn circuit_json(app: &QniApp) -> String {
    crate::url_circuit::circuit_to_json(
        &app.placed_gates,
        &app.circuit_blocks,
        &app.circuit_title,
        crate::qubit_count::QubitCount::try_new(app.required_visible_wire_count()).unwrap(),
    )
}

fn metrics(app: &QniApp) -> LayoutMetrics {
    layout_metrics(1280.0, app.layout_qubits(), app.min_circuit_slots())
}

fn to_screen(app: &QniApp, local: egui::Pos2) -> egui::Pos2 {
    local + app.circuit_origin(screen(), app.circuit_scroll_x).to_vec2()
}

fn slot(app: &QniApp, column: usize) -> egui::Pos2 {
    let metrics = metrics(app);
    to_screen(
        app,
        egui::pos2(metrics.slot_centers[column], metrics.line_ys[0]),
    )
}

fn block_body(app: &QniApp, index: usize) -> egui::Rect {
    let block = app.circuit_blocks.get(index).expect("test block exists");
    let local = circuit_block_rect(block, &metrics(app)).expect("block is laid out");
    local.translate(app.circuit_origin(screen(), 0.0).to_vec2())
}

fn top_label(app: &QniApp, index: usize) -> egui::Pos2 {
    let body = block_body(app, index);
    egui::pos2(body.center().x, body.top() - 10.0)
}

fn right_handle(app: &QniApp, index: usize) -> egui::Pos2 {
    let body = block_body(app, index);
    egui::pos2(body.right(), body.center().y)
}

fn left_handle(app: &QniApp, index: usize) -> egui::Pos2 {
    let body = block_body(app, index);
    egui::pos2(body.left(), body.center().y)
}

fn palette_block(app: &QniApp) -> egui::Pos2 {
    let index = (0..app.palette.len())
        .find(|&index| app.palette.item(index) == Some(PaletteItem::CircuitBlock))
        .expect("palette offers the comment block");
    let layout = app.palette.layout(screen().width());
    let local = app.palette.local_pos(index, &layout).unwrap();
    egui::pos2(palette_start_x(screen().width(), &layout), PALETTE_ROW_Y)
        + local.to_vec2()
        + egui::vec2(PALETTE_SIZE / 2.0, PALETTE_SIZE / 2.0)
}

fn frame(app: &mut QniApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    ctx.begin_pass(egui::RawInput {
        events,
        screen_rect: Some(screen()),
        ..Default::default()
    });
    app.handle_input(screen(), ctx, screen(), false);
    let _ = ctx.end_pass();
}

fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

fn press(app: &mut QniApp, ctx: &egui::Context, pos: egui::Pos2) {
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(pos), button(pos, true)],
    );
}

fn move_to(app: &mut QniApp, ctx: &egui::Context, pos: egui::Pos2) {
    frame(app, ctx, vec![egui::Event::PointerMoved(pos)]);
}

fn release(app: &mut QniApp, ctx: &egui::Context, pos: egui::Pos2) {
    frame(app, ctx, vec![button(pos, false)]);
}

fn drag(app: &mut QniApp, ctx: &egui::Context, from: egui::Pos2, to: egui::Pos2) {
    press(app, ctx, from);
    move_to(app, ctx, to);
    release(app, ctx, to);
}

fn click(app: &mut QniApp, ctx: &egui::Context, pos: egui::Pos2) {
    press(app, ctx, pos);
    release(app, ctx, pos);
}

fn drop_palette_block(app: &mut QniApp, ctx: &egui::Context, column: usize) {
    let from = palette_block(app);
    let to = slot(app, column);
    drag(app, ctx, from, to);
}

const H_CNOT_Z: &str = r#"{"cols":[["H"],["•","X"],["Z"]]}"#;

#[test]
fn dropping_the_palette_block_on_a_gate_column_wraps_that_column() {
    let (mut app, ctx) = editable(H_CNOT_Z);
    drop_palette_block(&mut app, &ctx, 0);

    assert_eq!(
        circuit_json(&app),
        r#"{"cols":[["{コメント"],["H"],["}"],["•","X"],["Z"]]}"#
    );
}

#[test]
fn dropping_the_palette_block_opens_its_label_editor() {
    let (mut app, ctx) = editable(H_CNOT_Z);
    drop_palette_block(&mut app, &ctx, 1);

    assert_eq!(
        app.circuit_block_editor.as_ref().map(|editor| (
            editor.index,
            editor.slot,
            editor.draft.as_str()
        )),
        Some((0, CircuitBlockLabelSlot::Top, "コメント"))
    );
}

#[test]
fn dropping_the_palette_block_on_the_trailing_area_adds_nothing() {
    let (mut app, ctx) = editable(H_CNOT_Z);
    drop_palette_block(&mut app, &ctx, 3);

    assert_eq!(circuit_json(&app), H_CNOT_Z);
}

#[test]
fn dropping_the_palette_block_inside_a_block_adds_nothing() {
    let json = r#"{"cols":[["{a"],["H"],["•","X"],["}"]]}"#;
    let (mut app, ctx) = editable(json);
    drop_palette_block(&mut app, &ctx, 1);

    assert_eq!(circuit_json(&app), json);
}

#[test]
fn dropping_the_palette_block_on_a_wide_gate_wraps_the_whole_gate() {
    let (mut app, ctx) = editable(r#"{"cols":[["H"],["Amps2"],[1],["X"]]}"#);
    drop_palette_block(&mut app, &ctx, 2);

    assert_eq!(
        circuit_json(&app),
        r#"{"cols":[["H"],["{コメント"],["Amps2"],[1],["}"],["X"]]}"#
    );
}

#[test]
fn undoing_a_dropped_block_removes_it() {
    let (mut app, ctx) = editable(H_CNOT_Z);
    drop_palette_block(&mut app, &ctx, 0);
    app.circuit_block_editor = None;
    app.undo_circuit(&ctx);

    assert_eq!(circuit_json(&app), H_CNOT_Z);
}

#[test]
fn locked_circuit_does_not_take_the_palette_block() {
    let (mut app, ctx) = editable(H_CNOT_Z);
    app.library.toggle_active_lock();
    drop_palette_block(&mut app, &ctx, 0);

    assert!(app.circuit_blocks.is_empty());
}

const BLOCK_THEN_GATES: &str = r#"{"cols":[["{a"],["H"],["}"],["•","X"],["Z"]]}"#;

#[test]
fn dragging_the_right_handle_one_column_widens_the_block() {
    let (mut app, ctx) = editable(BLOCK_THEN_GATES);
    let from = right_handle(&app, 0);
    drag(&mut app, &ctx, from, from + egui::vec2(56.0, 0.0));

    assert_eq!(
        circuit_json(&app),
        r#"{"cols":[["{a"],["H"],["•","X"],["}"],["Z"]]}"#
    );
}

#[test]
fn right_handle_stops_after_the_last_gate() {
    let (mut app, ctx) = editable(BLOCK_THEN_GATES);
    let from = right_handle(&app, 0);
    drag(&mut app, &ctx, from, from + egui::vec2(56.0 * 6.0, 0.0));

    assert_eq!(
        circuit_json(&app),
        r#"{"cols":[["{a"],["H"],["•","X"],["Z"],["}"]]}"#
    );
}

#[test]
fn right_handle_stops_at_the_next_block() {
    let (mut app, ctx) = editable(r#"{"cols":[["{a"],["H"],["}"],["X"],["{b"],["Z"],["}"]]}"#);
    let from = right_handle(&app, 0);
    drag(&mut app, &ctx, from, from + egui::vec2(56.0 * 3.0, 0.0));

    assert_eq!(
        circuit_json(&app),
        r#"{"cols":[["{a"],["H"],["X"],["}"],["{b"],["Z"],["}"]]}"#
    );
}

#[test]
fn left_handle_keeps_at_least_one_column() {
    let (mut app, ctx) = editable(r#"{"cols":[["H"],["{a"],["X"],["Z"],["}"]]}"#);
    let from = left_handle(&app, 0);
    drag(&mut app, &ctx, from, from + egui::vec2(56.0 * 4.0, 0.0));

    assert_eq!(
        circuit_json(&app),
        r#"{"cols":[["H"],["X"],["{a"],["Z"],["}"]]}"#
    );
}

#[test]
fn left_handle_does_not_stop_inside_a_wide_gate() {
    let (mut app, ctx) = editable(r#"{"cols":[["Amps2"],[1],["{a"],["X"],["}"]]}"#);
    let from = left_handle(&app, 0);
    drag(&mut app, &ctx, from, from - egui::vec2(56.0 * 1.2, 0.0));

    assert_eq!(
        circuit_json(&app),
        r#"{"cols":[["{a"],["Amps2"],[1],["X"],["}"]]}"#
    );
}

#[test]
fn resizing_is_one_undo_step() {
    let (mut app, ctx) = editable(BLOCK_THEN_GATES);
    let from = right_handle(&app, 0);
    drag(&mut app, &ctx, from, from + egui::vec2(56.0, 0.0));
    app.undo_circuit(&ctx);

    assert_eq!(circuit_json(&app), BLOCK_THEN_GATES);
}

#[test]
fn escape_cancels_a_resize() {
    let (mut app, ctx) = editable(BLOCK_THEN_GATES);
    let from = right_handle(&app, 0);
    press(&mut app, &ctx, from);
    move_to(&mut app, &ctx, from + egui::vec2(56.0, 0.0));
    escape(&mut app, &ctx);

    assert_eq!(circuit_json(&app), BLOCK_THEN_GATES);
}

#[test]
fn dragging_a_label_moves_the_block_keeping_its_width() {
    let (mut app, ctx) = editable(r#"{"cols":[["{a"],["H"],["X"],["}"],["Z"],["Y"]]}"#);
    let from = top_label(&app, 0);
    drag(&mut app, &ctx, from, from + egui::vec2(56.0 * 2.0, 0.0));

    assert_eq!(
        circuit_json(&app),
        r#"{"cols":[["H"],["X"],["{a"],["Z"],["Y"],["}"]]}"#
    );
}

fn escape(app: &mut QniApp, ctx: &egui::Context) {
    frame(
        app,
        ctx,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
}

const TOUCHING_BLOCKS: &str = r#"{"cols":[["{a"],["H"],["}"],["{b"],["X"],["}"],["Z"]]}"#;

#[test]
fn a_shared_edge_belongs_to_the_block_under_the_pointer() {
    let (mut app, ctx) = editable(TOUCHING_BLOCKS);
    let edge = left_handle(&app, 1) + egui::vec2(3.0, 0.0);
    move_to(&mut app, &ctx, edge);

    assert_eq!(
        app.hovered_circuit_block
            .map(|hover| (hover.index, hover.handle)),
        Some((1, Some(crate::app::CircuitBlockEdge::Left)))
    );
}

#[test]
fn a_label_move_jumps_over_a_block_it_would_overlap() {
    let (mut app, ctx) = editable(TOUCHING_BLOCKS);
    let from = top_label(&app, 0);
    drag(&mut app, &ctx, from, from + egui::vec2(56.0 * 1.6, 0.0));

    assert_eq!(
        circuit_json(&app),
        r#"{"cols":[["H"],["{b"],["X"],["}"],["{a"],["Z"],["}"]]}"#
    );
}

#[test]
fn a_label_move_stops_at_the_last_gate() {
    let (mut app, ctx) = editable(r#"{"cols":[["{a"],["H"],["}"],["X"]]}"#);
    let from = top_label(&app, 0);
    drag(&mut app, &ctx, from, from + egui::vec2(56.0 * 5.0, 0.0));

    assert_eq!(circuit_json(&app), r#"{"cols":[["H"],["{a"],["X"],["}"]]}"#);
}

#[test]
fn a_label_move_is_one_undo_step() {
    let (mut app, ctx) = editable(r#"{"cols":[["{a"],["H"],["}"],["X"],["Z"]]}"#);
    let from = top_label(&app, 0);
    drag(&mut app, &ctx, from, from + egui::vec2(56.0 * 2.0, 0.0));
    app.undo_circuit(&ctx);

    assert_eq!(
        circuit_json(&app),
        r#"{"cols":[["{a"],["H"],["}"],["X"],["Z"]]}"#
    );
}

#[test]
fn escape_cancels_a_label_move() {
    let json = r#"{"cols":[["{a"],["H"],["}"],["X"],["Z"]]}"#;
    let (mut app, ctx) = editable(json);
    let from = top_label(&app, 0);
    press(&mut app, &ctx, from);
    move_to(&mut app, &ctx, from + egui::vec2(56.0 * 2.0, 0.0));
    escape(&mut app, &ctx);

    assert_eq!(circuit_json(&app), json);
}

#[test]
fn escape_cancels_carrying_the_palette_block() {
    let (mut app, ctx) = editable(H_CNOT_Z);
    let from = palette_block(&app);
    let to = slot(&app, 0);
    press(&mut app, &ctx, from);
    move_to(&mut app, &ctx, to);
    escape(&mut app, &ctx);
    release(&mut app, &ctx, to);

    assert_eq!(circuit_json(&app), H_CNOT_Z);
}

#[test]
fn dragging_a_label_out_of_the_circuit_removes_only_the_block() {
    let (mut app, ctx) = editable(BLOCK_THEN_GATES);
    let from = top_label(&app, 0);
    drag(&mut app, &ctx, from, from - egui::vec2(0.0, 60.0));

    assert_eq!(circuit_json(&app), r#"{"cols":[["H"],["•","X"],["Z"]]}"#);
}

#[test]
fn clicking_a_label_opens_its_editor() {
    let (mut app, ctx) = editable(BLOCK_THEN_GATES);
    let label = top_label(&app, 0);
    click(&mut app, &ctx, label);

    assert_eq!(
        app.circuit_block_editor
            .as_ref()
            .map(|editor| editor.draft.as_str()),
        Some("a")
    );
}

#[test]
fn clicking_the_lower_label_edits_in_the_lower_slot() {
    let (mut app, ctx) = editable(BLOCK_THEN_GATES);
    let body = block_body(&app, 0);
    click(
        &mut app,
        &ctx,
        egui::pos2(body.center().x, body.bottom() + 10.0),
    );

    assert_eq!(
        app.circuit_block_editor.as_ref().map(|editor| editor.slot),
        Some(CircuitBlockLabelSlot::Bottom)
    );
}

#[test]
fn locked_circuit_labels_do_not_open_the_editor() {
    let (mut app, ctx) = editable(BLOCK_THEN_GATES);
    app.library.toggle_active_lock();
    let label = top_label(&app, 0);
    click(&mut app, &ctx, label);

    assert!(app.circuit_block_editor.is_none());
}

#[test]
fn embed_palette_without_the_comment_block_keeps_labels_read_only() {
    let (mut app, ctx) = embed(BLOCK_THEN_GATES, &["H", "X"]);
    let label = top_label(&app, 0);
    click(&mut app, &ctx, label);

    assert!(app.circuit_block_editor.is_none());
}

#[test]
fn embed_palette_without_the_comment_block_hides_the_handles() {
    let (mut app, ctx) = embed(BLOCK_THEN_GATES, &["H", "X"]);
    let handle = right_handle(&app, 0);
    move_to(&mut app, &ctx, handle);

    assert_eq!(app.hovered_circuit_block, None);
}

#[test]
fn embed_palette_with_the_comment_block_opens_the_editor() {
    let (mut app, ctx) = embed(BLOCK_THEN_GATES, &["H", "{}"]);
    let label = top_label(&app, 0);
    click(&mut app, &ctx, label);

    assert!(app.circuit_block_editor.is_some());
}

#[test]
fn hovering_the_right_edge_reveals_the_right_handle() {
    let (mut app, ctx) = editable(BLOCK_THEN_GATES);
    let handle = right_handle(&app, 0);
    move_to(&mut app, &ctx, handle);

    assert_eq!(
        app.hovered_circuit_block.and_then(|hover| hover.handle),
        Some(crate::app::CircuitBlockEdge::Right)
    );
}

#[test]
fn pressing_elsewhere_while_editing_commits_the_editor() {
    let (mut app, ctx) = editable(BLOCK_THEN_GATES);
    let label = top_label(&app, 0);
    click(&mut app, &ctx, label);
    let elsewhere = slot(&app, 2);
    press(&mut app, &ctx, elsewhere);

    assert!(app
        .circuit_block_editor
        .as_ref()
        .is_some_and(|editor| editor.commit_after_frame));
}
