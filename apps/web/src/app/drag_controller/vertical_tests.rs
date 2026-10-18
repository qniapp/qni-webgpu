use super::*;
use crate::app::circuit_history::CircuitRevision;
use crate::app::circuit_library;
use crate::layout::CircuitProjection;

const SCREEN: egui::Rect = egui::Rect {
    min: egui::pos2(8.0, 8.0),
    max: egui::pos2(382.0, 836.0),
};
const CONTENT: egui::Rect = egui::Rect {
    min: egui::pos2(8.0, 196.0),
    max: egui::pos2(382.0, 716.0),
};

fn fixture(json: &str) -> (QniApp, egui::Context) {
    let ctx = egui::Context::default();
    let mut app = QniApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()));
    app.load_circuit_json_into_editor(json, &ctx);
    app.library = circuit_library::for_startup(json.to_owned(), true).0;
    app.circuit_revision = CircuitRevision::starting_at(json.to_owned());
    app.orientation = CircuitOrientation::Vertical;
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

fn projection(app: &QniApp) -> CircuitProjection {
    app.circuit_projection(CONTENT, app.circuit_scroll_time)
}

/// Screen centre of the cell at `column` on `wire`.
fn cell(app: &QniApp, column: usize, wire: usize) -> egui::Pos2 {
    let metrics = CircuitInputGeometry::for_app(app, CONTENT, SCREEN).metrics;
    projection(app).to_screen(egui::pos2(
        metrics.slot_centers[column],
        metrics.line_ys[wire],
    ))
}

fn carousel(app: &QniApp) -> PaletteCarousel {
    PaletteCarousel::new(SCREEN, app.palette.len())
}

fn frame(app: &mut QniApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    ctx.begin_pass(egui::RawInput {
        events,
        ..Default::default()
    });
    app.handle_input(CONTENT, ctx, SCREEN, false);
    let _ = ctx.end_pass();
}

fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::default(),
    }
}

fn press(app: &mut QniApp, ctx: &egui::Context, pos: egui::Pos2) {
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(pos), button(pos, true)],
    );
}

fn release(app: &mut QniApp, ctx: &egui::Context, pos: egui::Pos2) {
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(pos), button(pos, false)],
    );
}

fn drag(app: &mut QniApp, ctx: &egui::Context, from: egui::Pos2, to: egui::Pos2) {
    press(app, ctx, from);
    for step in 1..=4 {
        let pos = from + (to - from) * (step as f32 / 4.0);
        frame(app, ctx, vec![egui::Event::PointerMoved(pos)]);
    }
    release(app, ctx, to);
}

fn tap(app: &mut QniApp, ctx: &egui::Context, pos: egui::Pos2) {
    press(app, ctx, pos);
    release(app, ctx, pos);
}

#[test]
fn vertical_wire0_sits_one_step_outset_inside_the_right_edge() {
    let (app, _) = fixture(r#"{"cols":[]}"#);

    assert_eq!(
        cell(&app, 0, 0).x,
        CONTENT.max.x - crate::constants::VERTICAL_WIRE0_RIGHT_INSET
    );
}

#[test]
fn vertical_drop_on_q0_step0_writes_h() {
    let (mut app, ctx) = fixture(r#"{"cols":[]}"#);
    let from = carousel(&app).slot_rect(0).center();
    let to = cell(&app, 0, 0);
    drag(&mut app, &ctx, from, to);

    assert_eq!(circuit_json(&app), r#"{"cols":[["H"]]}"#);
}

#[test]
fn vertical_drop_on_q1_writes_second_wire() {
    let (mut app, ctx) = fixture(r#"{"cols":[]}"#);
    let from = carousel(&app).slot_rect(1).center();
    let to = cell(&app, 0, 1);
    drag(&mut app, &ctx, from, to);

    assert_eq!(circuit_json(&app), r#"{"cols":[[1,"X"]]}"#);
}

#[test]
fn vertical_drop_on_second_step_writes_second_column() {
    let (mut app, ctx) = fixture(r#"{"cols":[["H"]]}"#);
    let from = carousel(&app).slot_rect(3).center();
    let to = cell(&app, 1, 0);
    drag(&mut app, &ctx, from, to);

    assert_eq!(circuit_json(&app), r#"{"cols":[["H"],["Z"]]}"#);
}

#[test]
fn vertical_drop_left_of_all_wires_removes_gate() {
    let (mut app, ctx) = fixture(r#"{"cols":[["H"]]}"#);
    let from = cell(&app, 0, 0);
    let to = egui::pos2(CONTENT.min.x + 16.0, from.y);
    drag(&mut app, &ctx, from, to);

    assert_eq!(circuit_json(&app), r#"{"cols":[]}"#);
}

#[test]
fn vertical_drag_moves_gate_to_the_next_wire() {
    let (mut app, ctx) = fixture(r#"{"cols":[["H"]]}"#);
    let from = cell(&app, 0, 0);
    let to = cell(&app, 0, 1);
    drag(&mut app, &ctx, from, to);

    assert_eq!(circuit_json(&app), r#"{"cols":[[1,"H"]]}"#);
}

#[test]
fn vertical_next_arrow_advances_one_page() {
    let (mut app, ctx) = fixture(r#"{"cols":[]}"#);
    let next = carousel(&app).next_rect().center();
    tap(&mut app, &ctx, next);

    assert_eq!(app.palette_page, 1);
}

#[test]
fn vertical_carousel_picks_up_from_the_shown_page() {
    let (mut app, ctx) = fixture(r#"{"cols":[]}"#);
    app.palette_page = 1;
    let from = carousel(&app).slot_rect(1).center();
    let to = cell(&app, 0, 0);
    drag(&mut app, &ctx, from, to);

    assert_eq!(circuit_json(&app), r#"{"cols":[["S"]]}"#);
}

#[test]
fn vertical_step_press_does_not_select_before_release() {
    let (mut app, ctx) = fixture(r#"{"cols":[["H"],["Z"]]}"#);
    let step0 = cell(&app, 0, 0) + egui::vec2(crate::constants::GATE_SIZE, 0.0);
    press(&mut app, &ctx, step0);

    assert_eq!(app.breakpoint_step, Some(CircuitColumnIndex::new(1)));
}

#[test]
fn vertical_step_tap_selects_on_release() {
    let (mut app, ctx) = fixture(r#"{"cols":[["H"],["Z"]]}"#);
    let step0 = cell(&app, 0, 0) + egui::vec2(crate::constants::GATE_SIZE, 0.0);
    tap(&mut app, &ctx, step0);

    assert_eq!(app.breakpoint_step, Some(CircuitColumnIndex::ZERO));
}

#[test]
fn vertical_swipe_across_a_step_keeps_the_selection() {
    let (mut app, ctx) = fixture(r#"{"cols":[["H"],["Z"]]}"#);
    let step0 = cell(&app, 0, 0) + egui::vec2(crate::constants::GATE_SIZE, 0.0);
    drag(&mut app, &ctx, step0, step0 - egui::vec2(0.0, 120.0));

    assert_eq!(app.breakpoint_step, Some(CircuitColumnIndex::new(1)));
}

#[test]
fn vertical_swipe_up_pans_a_long_circuit() {
    let (mut app, ctx) = fixture(&long_circuit("H"));
    let from = egui::pos2(CONTENT.min.x + 40.0, CONTENT.max.y - 40.0);
    drag(&mut app, &ctx, from, from - egui::vec2(0.0, 200.0));

    assert_eq!(app.circuit_scroll_time, 200.0);
}

#[test]
fn vertical_pan_stops_at_the_circuit_end() {
    let (mut app, ctx) = fixture(&long_circuit("H"));
    let from = egui::pos2(CONTENT.min.x + 40.0, CONTENT.max.y - 40.0);
    drag(&mut app, &ctx, from, egui::pos2(from.x, -2000.0));
    let geometry = CircuitInputGeometry::for_app(&app, CONTENT, SCREEN);

    assert_eq!(
        app.circuit_scroll_time,
        app.max_circuit_scroll(CONTENT, &geometry.metrics)
    );
}

#[test]
fn vertical_press_on_the_palette_card_ignores_gates_scrolled_under_it() {
    let (mut app, ctx) = fixture(&long_circuit("X"));
    // Column 8's gate reaches past the circuit area into the card's top
    // padding, where the carousel has no target.
    let pos = egui::pos2(cell(&app, 8, 0).x, carousel(&app).rect().min.y + 2.0);
    press(&mut app, &ctx, pos);

    assert!(app.dragging.is_none());
}

fn long_circuit(gate: &str) -> String {
    format!(
        r#"{{"cols":[{}]}}"#,
        vec![format!(r#"["{gate}"]"#); 16].join(",")
    )
}

#[test]
fn vertical_drop_on_the_palette_card_does_not_fill_a_hidden_column() {
    let json = long_circuit("H");
    let (mut app, ctx) = fixture(&json);
    let card = carousel(&app);
    // Between two carousel slots, over wire 1 of a column scrolled under
    // the card.
    let to = egui::pos2(cell(&app, 9, 1).x, card.slot_rect(0).center().y);
    drag(&mut app, &ctx, card.slot_rect(1).center(), to);

    assert_eq!(circuit_json(&app), json);
}

#[test]
fn vertical_hover_on_the_palette_card_ignores_hidden_gates() {
    let (mut app, ctx) = fixture(&long_circuit("H"));
    let pos = egui::pos2(cell(&app, 8, 0).x, carousel(&app).rect().min.y + 2.0);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(pos)]);

    assert_eq!(app.hovered_gate_id, None);
}

#[test]
fn vertical_angle_label_under_the_carousel_does_not_take_the_arrow_tap() {
    let (mut app, ctx) = fixture(&long_circuit("Rx"));
    // Column 8's angle label sits under the top edge of the next arrow.
    let next = carousel(&app).next_rect();
    let pos = egui::pos2(cell(&app, 8, 0).x + 8.0, next.min.y + 2.0);
    tap(&mut app, &ctx, pos);

    assert_eq!(app.palette_page, 1);
}

#[test]
fn vertical_rendered_gate_bodies_snapshot() {
    let (app, ctx) = fixture(r#"{"cols":[["H"],["•","X"],["Measure","Measure"]]}"#);
    let geometry = CircuitInputGeometry::for_app(&app, CONTENT, SCREEN);
    ctx.begin_pass(egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_max(
            egui::Pos2::ZERO,
            egui::pos2(390.0, 844.0),
        )),
        ..Default::default()
    });
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("vertical-test"),
    ));
    let colors = app.colors();
    app.draw_circuit(
        &painter,
        CONTENT,
        &geometry.metrics,
        &colors,
        false,
        None,
        0.0,
    );
    let bodies = ctx
        .end_pass()
        .shapes
        .into_iter()
        .filter_map(|clipped| match clipped.shape {
            egui::Shape::Rect(shape) if shape.fill == colors.box_fill => Some(shape.rect),
            egui::Shape::Circle(shape) if shape.fill == colors.box_fill => Some(
                egui::Rect::from_center_size(shape.center, egui::Vec2::splat(shape.radius * 2.0)),
            ),
            _ => None,
        })
        .collect::<Vec<_>>();

    insta::assert_debug_snapshot!(bodies);
}

#[test]
fn vertical_wire_and_step_bar_segments_snapshot() {
    let (app, ctx) = fixture(r#"{"cols":[["H"],["•","X"]]}"#);
    let geometry = CircuitInputGeometry::for_app(&app, CONTENT, SCREEN);
    ctx.begin_pass(egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_max(
            egui::Pos2::ZERO,
            egui::pos2(390.0, 844.0),
        )),
        ..Default::default()
    });
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("vertical-lines"),
    ));
    let colors = app.colors();
    app.draw_circuit(
        &painter,
        CONTENT,
        &geometry.metrics,
        &colors,
        false,
        None,
        0.0,
    );
    let segments = ctx
        .end_pass()
        .shapes
        .into_iter()
        .filter_map(|clipped| match clipped.shape {
            egui::Shape::LineSegment { points, stroke } => Some((points, stroke.width)),
            _ => None,
        })
        .collect::<Vec<_>>();

    insta::assert_debug_snapshot!(segments);
}
