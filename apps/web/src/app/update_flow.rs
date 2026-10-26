//! Per-frame update coordinator for `QniApp`.
//!
//! The app root owns state; this module owns the order of each frame:
//! circuit input/draw, state-panel interactions, GPU plan refresh, overlay
//! drawing, and repaint / debug-HUD tail work.

mod circuit_frame;
mod frame_tail;
mod gpu_frame;
mod overlay;
mod state_panel_frame;

use eframe::{egui, egui_wgpu};

use super::{GateId, QniApp};
use crate::gpu::StateVectorResourceBootstrapCallback;
use crate::layout::CircuitOrientation;
use crate::render::StatePanelLayout;
use crate::shared::now_seconds;

struct CircuitFrameState {
    content_rect: Option<egui::Rect>,
    gpu_batch: Option<crate::render::CircuitGpuBatch>,
    recompute: bool,
    dragging_gate_id: Option<GateId>,
    live_drag_gpu_overlay_ready: bool,
    insert_preview_painted: bool,
}

struct StatePanelFrameState {
    state_count: usize,
    recompute: bool,
    layout: StatePanelLayout,
}

impl QniApp {
    fn show_update_central_panel(
        &mut self,
        ctx: &egui::Context,
        frame: &mut eframe::Frame,
        ui: &mut egui::Ui,
    ) {
        let screen_rect = ui.max_rect();
        self.update_orientation(ctx);
        let colors = self.colors();
        let target_format = frame.wgpu_render_state().map(|state| state.target_format);

        // Decide whether wheel-over-the-panel should suppress the surrounding
        // ScrollArea's page-scroll. If pointer is on the state panel (or its
        // popover), route wheel to our aspect / zoom handlers instead.
        let pointer_over_state_panel = self.compute_state_panel_input_gate(ctx, screen_rect);

        if let Some(target_format) = target_format {
            if self.external_gpu_display_resources_needed() {
                let callback = StateVectorResourceBootstrapCallback { target_format };
                let paint_callback = egui_wgpu::Callback::new_paint_callback(screen_rect, callback);
                ui.painter().add(egui::Shape::Callback(paint_callback));
            }
        }

        let circuit_frame = self.show_circuit_scroll_area(
            ctx,
            ui,
            screen_rect,
            target_format,
            pointer_over_state_panel,
            &colors,
        );
        let mut state_frame = self.prepare_state_panel_frame(screen_rect);
        self.process_state_panel_interactions(ctx, ui, screen_rect, &state_frame);
        // State-panel interactions can change zoom, aspect, viewport size, or
        // panel offset. Draw with a fresh layout in the same frame; otherwise
        // the pan/anchor math is one frame ahead of radius/cell_pitch and the
        // GPU-rendered circles visibly wobble during wheel zoom.
        state_frame = self.prepare_state_panel_frame(screen_rect);
        #[cfg(target_arch = "wasm32")]
        if !self.mode.uses_browser_state() {
            if let Some(content_rect) = circuit_frame.content_rect {
                self.publish_embed_touch_targets(
                    ctx,
                    content_rect,
                    screen_rect,
                    &state_frame.layout,
                );
            }
        }
        self.draw_frame_overlay(
            ctx,
            frame,
            screen_rect,
            &colors,
            &circuit_frame,
            &state_frame,
        );
    }
}

impl QniApp {
    /// Decides the layout once per frame. Embeds keep the horizontal
    /// layout, and a gesture in progress keeps the layout it started in
    /// (a phone keyboard or URL bar may resize the canvas mid-gesture).
    fn update_orientation(&mut self, ctx: &egui::Context) {
        if self.dragging.is_some()
            || self.span_resize_drag.is_some()
            || self.angle_editor.is_some()
            || self.circuit_pan.is_some()
        {
            return;
        }
        let orientation = if matches!(self.mode, super::AppMode::Embed { .. }) {
            CircuitOrientation::Horizontal
        } else {
            CircuitOrientation::for_canvas(ctx.content_rect().size())
        };
        if orientation != self.orientation {
            self.orientation = orientation;
            self.circuit_scroll_time = 0.0;
            self.state_panel.offset = egui::Vec2::ZERO;
            ctx.request_repaint();
        }
    }
}

impl eframe::App for QniApp {
    #[cfg(target_arch = "wasm32")]
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }

    fn persist_egui_memory(&self) -> bool {
        self.mode.uses_browser_state()
    }

    fn ui(&mut self, root_ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = &root_ui.ctx().clone();
        self.cancel_touch_gesture(ctx);
        crate::icons::set_sdf_target_format(
            ctx,
            frame.wgpu_render_state().map(|state| state.target_format),
        );
        let frame_start = now_seconds();
        self.apply_pending_circuit_library_seed(ctx);
        self.apply_external_circuit_library_update(ctx);
        self.apply_pending_url_payload(ctx);
        let colors = self.colors();
        let mut panel_frame = egui::Frame::central_panel(&ctx.global_style());
        panel_frame.fill = colors.background;
        egui::CentralPanel::default()
            .frame(panel_frame)
            .show(root_ui, |ui| {
                self.show_update_central_panel(ctx, frame, ui);
            });
        self.show_exec_mode_toolbar(ctx, &colors);
        self.finish_update_frame(ctx, frame_start);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PHONE: egui::Vec2 = egui::vec2(390.0, 844.0);
    const DESKTOP: egui::Vec2 = egui::vec2(1000.0, 800.0);

    fn app() -> (QniApp, egui::Context) {
        let ctx = egui::Context::default();
        let app = QniApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()));
        (app, ctx)
    }

    fn update(app: &mut QniApp, ctx: &egui::Context, canvas: egui::Vec2) {
        let _ = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, canvas)),
                ..Default::default()
            },
            |ui| app.update_orientation(ui.ctx()),
        );
    }

    #[test]
    fn phone_canvas_selects_the_vertical_layout() {
        let (mut app, ctx) = app();
        update(&mut app, &ctx, PHONE);

        assert_eq!(app.orientation, CircuitOrientation::Vertical);
    }

    #[test]
    fn desktop_canvas_returns_to_the_horizontal_layout() {
        let (mut app, ctx) = app();
        update(&mut app, &ctx, PHONE);
        update(&mut app, &ctx, DESKTOP);

        assert_eq!(app.orientation, CircuitOrientation::Horizontal);
    }

    #[test]
    fn embed_keeps_the_horizontal_layout_on_a_phone() {
        let (mut app, ctx) = app();
        app.mode = super::super::AppMode::Embed {
            show_state_panel: true,
            max_wire_count: None,
        };
        update(&mut app, &ctx, PHONE);

        assert_eq!(app.orientation, CircuitOrientation::Horizontal);
    }

    #[test]
    fn layout_is_kept_while_a_pan_is_in_progress() {
        let (mut app, ctx) = app();
        update(&mut app, &ctx, PHONE);
        app.circuit_pan = Some(super::super::drag_controller::pan::CircuitPan::new(
            egui::pos2(60.0, 400.0),
            None,
        ));
        update(&mut app, &ctx, DESKTOP);

        assert_eq!(app.orientation, CircuitOrientation::Vertical);
    }

    #[test]
    fn switching_layout_resets_the_time_scroll() {
        let (mut app, ctx) = app();
        app.circuit_scroll_time = 120.0;
        update(&mut app, &ctx, PHONE);

        assert_eq!(app.circuit_scroll_time, 0.0);
    }
}
