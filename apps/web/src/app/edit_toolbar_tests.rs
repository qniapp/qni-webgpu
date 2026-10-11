//! Clicks on the edit cluster (Undo / Redo / Clear) of the embed and
//! standalone toolbars.

use eframe::egui;

use super::{CircuitRevision, EmbedStartup, QniApp};

const H: &str = r#"{"cols":[["H"]]}"#;
const EMPTY: &str = r#"{"cols":[]}"#;
// Button centres: 32px buttons with gap-2 = 8px between them.
const UNDO_X: f32 = 16.0;
const REDO_X: f32 = 56.0;
const THIRD_BUTTON_X: f32 = 96.0;

fn embed() -> (QniApp, egui::Context) {
    let ctx = egui::Context::default();
    let cc = eframe::CreationContext::_new_kittest(ctx.clone());
    let startup = EmbedStartup::parse::<&str>(H, true, None, None).unwrap();
    (QniApp::new_with_startup(&cc, Some(startup)), ctx)
}

fn standalone() -> (QniApp, egui::Context) {
    let ctx = egui::Context::default();
    let mut app = QniApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()));
    // Start from an editable user circuit rather than the locked sample.
    let (library, _) = super::circuit_library::for_startup(H.into(), true);
    app.library = library;
    app.load_circuit_json_into_editor(H, &ctx);
    app.circuit_revision = CircuitRevision::starting_at(H.into());
    (app, ctx)
}

fn remove_all_gates(app: &mut QniApp, ctx: &egui::Context) {
    app.placed_gates.clear();
    app.commit_current_circuit(ctx);
}

fn click_toolbar(app: &mut QniApp, ctx: &egui::Context, x: f32) {
    let draw = |app: &mut QniApp, input: egui::RawInput| {
        let mut origin = egui::Pos2::ZERO;
        let _ = ctx.run_ui(input, |root_ui| {
            egui::CentralPanel::default().show(root_ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0); // gap-2 = 8px.
                    origin = ui.cursor().min;
                    app.show_test_edit_utilities(ui);
                });
            });
        });
        origin
    };
    let position = draw(app, Default::default()) + egui::vec2(x, 16.0);
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
        let _ = draw(app, input);
    }
}

fn active_json(app: &QniApp) -> &str {
    &app.library.active().circuit_json
}

#[test]
fn embed_toolbar_has_no_clear_button() {
    let (mut app, ctx) = embed();
    click_toolbar(&mut app, &ctx, THIRD_BUTTON_X);
    assert_eq!(active_json(&app), H);
}

#[test]
fn embed_toolbar_undo_restores_removed_gates() {
    let (mut app, ctx) = embed();
    remove_all_gates(&mut app, &ctx);
    click_toolbar(&mut app, &ctx, UNDO_X);
    assert_eq!(active_json(&app), H);
}

#[test]
fn embed_toolbar_redo_reapplies_undone_edit() {
    let (mut app, ctx) = embed();
    remove_all_gates(&mut app, &ctx);
    app.undo_circuit(&ctx);
    click_toolbar(&mut app, &ctx, REDO_X);
    assert_eq!(active_json(&app), EMPTY);
}

#[test]
fn standalone_toolbar_clear_button_clears_circuit() {
    let (mut app, ctx) = standalone();
    click_toolbar(&mut app, &ctx, THIRD_BUTTON_X);
    assert_eq!(active_json(&app), EMPTY);
}
