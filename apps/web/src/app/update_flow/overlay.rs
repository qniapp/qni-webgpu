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
        self.process_gpu_recompute(
            target_format,
            state_frame.recompute,
            state_frame.state_count,
            ctx,
        );

        if let Some(content_rect) = circuit_frame.content_rect {
            let projection = self.circuit_projection(content_rect, self.circuit_scroll_time);
            self.draw_survival_labels(
                &overlay_painter,
                screen_rect,
                projection,
                colors,
                self.state_panel_visible().then(|| {
                    state_frame
                        .layout
                        .state_rect
                        .translate(self.state_panel.offset)
                }),
            );
        }
        self.draw_palette(&overlay_painter, screen_rect, colors);
        if self.state_panel_visible() {
            let svp_t0 = now_seconds();
            self.draw_state_vector(
                &overlay_painter,
                colors,
                &state_frame.layout,
                self.state_panel.offset,
                state_frame.layout.handle_height,
                screen_rect,
                target_format,
            );
            if self.fps_hud_visible {
                let svp_secs = (now_seconds() - svp_t0).max(0.0) as f32;
                self.fps_hud_svp_history.push_back(svp_secs);
                while self.fps_hud_svp_history.len() > 120 {
                    self.fps_hud_svp_history.pop_front();
                }
            }
        }
        if !self.state_panel_visible() && self.local_state_vector_active() {
            if let Some(target_format) = target_format {
                // Hidden panels still maintain GPU snapshots for displays and the runner API.
                let callback = crate::gpu::StateVectorComputeCallback {
                    update: self.state_vector_update(),
                    target_format,
                };
                overlay_painter.add(egui::Shape::Callback(
                    eframe::egui_wgpu::Callback::new_paint_callback(screen_rect, callback),
                ));
            }
        }
        if let (Some(content_rect), Some(dragging_gate_id)) =
            (circuit_frame.content_rect, circuit_frame.dragging_gate_id)
        {
            let projection = self.circuit_projection(content_rect, self.circuit_scroll_time);
            if let Some(batch) = circuit_frame
                .gpu_batch
                .as_ref()
                .filter(|_| circuit_frame.insert_preview_painted)
            {
                self.draw_insert_preview_overlay(
                    &overlay_painter,
                    batch,
                    projection,
                    colors,
                    dragging_gate_id,
                );
            } else {
                self.draw_drag_preview(
                    &overlay_painter,
                    projection,
                    colors,
                    dragging_gate_id,
                    circuit_frame.live_drag_gpu_overlay_ready,
                );
            }
        }
        if let Some(content_rect) = circuit_frame.content_rect {
            let projection = self.circuit_projection(content_rect, self.circuit_scroll_time);
            self.draw_circuit_block_drag_overlay(&overlay_painter, projection, colors);
            // Paint display popovers in the foreground overlay so they sit above
            // the palette. Use the full screen rect as the GPU value viewport:
            // scrolled circuit content can have a negative rect origin, while
            // egui clamps callback viewports to the visible screen.
            self.draw_bloch_hover_popup(
                &overlay_painter,
                screen_rect,
                projection,
                circuit_frame.dragging_gate_id,
                colors,
            );
            self.draw_probability_hover_popup(
                &overlay_painter,
                screen_rect,
                projection,
                circuit_frame.dragging_gate_id,
                colors,
            );
            self.draw_amplitude_hover_popup(
                &overlay_painter,
                screen_rect,
                projection,
                circuit_frame.dragging_gate_id,
                colors,
            );
        }

        // Tooltip is drawn last so it sits on top of the drag preview / state
        // panel / everything else in the overlay layer.
        self.draw_palette_tooltip(&overlay_painter, screen_rect, colors);
    }
}
