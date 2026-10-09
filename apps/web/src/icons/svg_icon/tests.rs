use super::{texture_id, GateGlyph};
use eframe::egui;

#[test]
fn cold_texture_is_allocated_once_and_reused() {
    let ctx = egui::Context::default();
    ctx.begin_pass(egui::RawInput::default());
    let first = texture_id(&ctx, GateGlyph::H);
    let second = texture_id(&ctx, GateGlyph::H);
    let output = ctx.end_pass();
    let uploads = output
        .textures_delta
        .set
        .iter()
        .filter(|(id, _)| *id == first)
        .count();
    assert_eq!((first == second, uploads), (true, 1));
}

#[test]
fn each_editor_allocates_its_own_glyph_texture() {
    let first = egui::Context::default();
    let second = egui::Context::default();
    first.begin_pass(egui::RawInput::default());
    texture_id(&first, GateGlyph::H);
    let _ = first.end_pass();
    second.begin_pass(egui::RawInput::default());
    let id = texture_id(&second, GateGlyph::H);
    let output = second.end_pass();
    assert!(output
        .textures_delta
        .set
        .iter()
        .any(|(uploaded, _)| *uploaded == id));
}
