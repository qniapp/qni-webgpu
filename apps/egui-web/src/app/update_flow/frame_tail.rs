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
