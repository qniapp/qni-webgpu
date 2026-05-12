use eframe::egui;

use super::{CircuitFrameState, StatePanelFrameState};
use crate::app::QniApp;
use crate::colors::Colors;
use crate::shared::now_seconds;

impl QniApp {
    pub(super) fn draw_frame_overlay(
        &mut self,
        ctx: &egui::Context,
        frame: &mut eframe::Frame,
        screen_rect: egui::Rect,
        colors: &Colors,
        circuit_frame: &CircuitFrameState,
        state_frame: &StatePanelFrameState,
    ) {
        let overlay_painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("overlay"),
        ));
        let target_format = frame.wgpu_render_state().map(|state| state.target_format);
        let recompute = self.process_gpu_recompute(
            target_format,
            state_frame.recompute,
            state_frame.state_count,
            ctx,
        );

        self.draw_palette(&overlay_painter, screen_rect, colors);
        let svp_t0 = now_seconds();
        self.draw_state_vector(
            &overlay_painter,
            colors,
            &state_frame.layout,
            self.state_panel.offset,
            state_frame.layout.handle_height,
            screen_rect,
            recompute,
            target_format,
        );
        if self.fps_hud_visible {
            let svp_secs = (now_seconds() - svp_t0).max(0.0) as f32;
            self.fps_hud_svp_history.push_back(svp_secs);
            while self.fps_hud_svp_history.len() > 120 {
                self.fps_hud_svp_history.pop_front();
            }
        }
        if let (Some(content_rect), Some(dragging_gate_id)) =
            (circuit_frame.content_rect, circuit_frame.dragging_gate_id)
        {
            self.draw_drag_preview(
                &overlay_painter,
                content_rect,
                colors,
                dragging_gate_id,
                self.circuit_scroll_x,
            );
        }

        // Tooltip is drawn last so it sits on top of the drag preview / state
        // panel / everything else in the overlay layer.
        self.draw_palette_tooltip(&overlay_painter, screen_rect, colors);
    }
}
