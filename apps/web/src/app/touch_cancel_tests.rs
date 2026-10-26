use super::*;

fn press() -> egui::Event {
    egui::Event::PointerButton {
        pos: egui::pos2(100.0, 100.0),
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::default(),
    }
}

fn touch(phase: egui::TouchPhase) -> egui::Event {
    egui::Event::Touch {
        device_id: egui::TouchDeviceId(0),
        id: egui::TouchId(1),
        phase,
        pos: egui::pos2(100.0, 100.0),
        force: None,
    }
}

fn cancellation_frame(events: Vec<egui::Event>) -> (bool, bool, usize) {
    let ctx = egui::Context::default();
    let mut app = QniApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()));
    let mut result = None;
    let _ = ctx.run_ui(
        egui::RawInput {
            events,
            ..Default::default()
        },
        |ui| {
            app.cancel_touch_gesture(ui.ctx());
            result = Some(ui.ctx().input(|input| {
                (
                    input.pointer.primary_down(),
                    input.pointer.any_click(),
                    input
                        .events
                        .iter()
                        .filter(|event| matches!(event, egui::Event::Touch { .. }))
                        .count(),
                )
            }));
        },
    );
    result.unwrap()
}

#[test]
fn cancellation_clears_pointer_without_clicking_or_dropping_touch_events() {
    assert_eq!(
        cancellation_frame(vec![
            press(),
            touch(egui::TouchPhase::Start),
            touch(egui::TouchPhase::Cancel)
        ]),
        (false, false, 2)
    );
}

#[test]
fn cancellation_does_not_clear_a_new_touch_in_the_same_frame() {
    assert_eq!(
        cancellation_frame(vec![
            press(),
            touch(egui::TouchPhase::Start),
            touch(egui::TouchPhase::Cancel),
            press(),
            touch(egui::TouchPhase::Start),
        ]),
        (true, false, 3)
    );
}

#[test]
fn ordinary_press_is_not_cancelled() {
    assert_eq!(
        cancellation_frame(vec![press(), touch(egui::TouchPhase::Start)]),
        (true, false, 1)
    );
}
