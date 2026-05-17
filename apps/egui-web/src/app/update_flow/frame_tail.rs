use eframe::egui;
use std::time::Duration;

use crate::app::QniApp;
use crate::constants::DRAG_REPAINT_MIN_SECS;
use crate::shared::now_seconds;

impl QniApp {
    pub(super) fn finish_update_frame(&mut self, ctx: &egui::Context, frame_start: f64) {
        let now = now_seconds();
        let frame_secs = (now - frame_start).max(0.0);
        if self.dragging.is_some() {
            self.schedule_drag_repaint(ctx, frame_secs);
        } else if self.drag_repaint_deadline.is_some() || self.drag_repaint_pending {
            self.drag_repaint_deadline = None;
            self.drag_repaint_pending = false;
        }
        if now < self.startup_repaint_until {
            ctx.request_repaint_after(Duration::from_secs_f64(DRAG_REPAINT_MIN_SECS));
        }

        self.process_fps_hud(ctx, frame_secs);
        self.publish_circuit_library_snapshot();
        self.publish_hover_snapshot();
    }

    fn publish_hover_snapshot(&self) {
        publish_hover_snapshot(
            self.hovered_gate_id,
            self.hovered_palette_index,
            self.hovered_chance_outcome,
        );
    }

    fn process_fps_hud(&mut self, ctx: &egui::Context, frame_secs: f64) {
        if ctx.input(|i| i.key_pressed(egui::Key::Backtick)) {
            self.fps_hud_visible = !self.fps_hud_visible;
            if !self.fps_hud_visible {
                self.fps_hud_history.clear();
                self.fps_hud_cpu_history.clear();
                self.fps_hud_svp_history.clear();
            }
        }
        if self.fps_hud_visible {
            self.draw_fps_hud(ctx, frame_secs);
        }
    }
}

#[cfg(all(target_arch = "wasm32", debug_assertions))]
fn publish_hover_snapshot(
    hovered_gate_id: Option<u32>,
    hovered_palette_index: Option<usize>,
    hovered_chance_outcome: Option<(u32, u32)>,
) {
    let gate = hovered_gate_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| "null".to_owned());
    let palette = hovered_palette_index
        .map(|index| index.to_string())
        .unwrap_or_else(|| "null".to_owned());
    let chance = hovered_chance_outcome
        .map(|(gate_id, outcome)| format!("{{\"gateId\":{gate_id},\"outcome\":{outcome}}}"))
        .unwrap_or_else(|| "null".to_owned());
    let snapshot = format!(
        "{{\"hoveredGateId\":{gate},\"hoveredPaletteIndex\":{palette},\"hoveredChanceOutcome\":{chance}}}"
    );
    crate::test_hooks::set_window_value(
        crate::test_hooks::QNI_HOVER_SNAPSHOT_JSON,
        &wasm_bindgen::JsValue::from_str(&snapshot),
    );
}

#[cfg(any(not(target_arch = "wasm32"), not(debug_assertions)))]
fn publish_hover_snapshot(
    _hovered_gate_id: Option<u32>,
    _hovered_palette_index: Option<usize>,
    _hovered_chance_outcome: Option<(u32, u32)>,
) {
}
