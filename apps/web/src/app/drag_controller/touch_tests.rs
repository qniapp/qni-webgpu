use super::*;
use crate::app::embed::EmbedStartup;

#[test]
fn horizontal_embed_touch_swipe_pans_the_time_axis() {
    let ctx = egui::Context::default();
    let json = format!(
        r#"{{"cols":[["H",1,1,"X"],{}]}}"#,
        vec![r#"["X"]"#; 13].join(",")
    );
    let startup = EmbedStartup::parse(&json, false, Some(&["H", "X"]), Some(4.0)).unwrap();
    let mut app = QniApp::new_with_startup(
        &eframe::CreationContext::_new_kittest(ctx.clone()),
        Some(startup),
    );
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(390.0, 440.0));
    let from = egui::pos2(330.0, 295.0);
    let to = from - egui::vec2(200.0, 0.0);
    for (pos, phase, pressed) in [
        (from, egui::TouchPhase::Start, Some(true)),
        (to, egui::TouchPhase::Move, None),
        (to, egui::TouchPhase::End, Some(false)),
    ] {
        let mut events = vec![
            egui::Event::Touch {
                device_id: egui::TouchDeviceId(1),
                id: egui::TouchId(1),
                phase,
                pos,
                force: None,
            },
            egui::Event::PointerMoved(pos),
        ];
        if let Some(pressed) = pressed {
            events.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::default(),
            });
        }
        ctx.begin_pass(egui::RawInput {
            screen_rect: Some(rect),
            events,
            ..Default::default()
        });
        app.handle_input(rect, &ctx, rect, false);
        let _ = ctx.end_pass();
    }

    assert_eq!(app.circuit_scroll_time, 200.0);
}
