//! Per-frame update coordinator for `QniApp`.
//!
//! The app root owns state; this module owns the order of each frame:
//! circuit input/draw, state-panel interactions, GPU plan refresh, overlay
//! drawing, and repaint / debug-HUD tail work.

use eframe::egui;
use std::time::Duration;

use super::QniApp;
use crate::colors::Colors;
use crate::constants::{
    state_circle_default_aspect_index, state_circle_layout, state_grid_zoom_limits,
    DRAG_REPAINT_MIN_SECS, MAX_QUBITS,
};
use crate::gpu::{MAX_BLOCH_SLOTS, MAX_MEASUREMENT_SLOTS, MAX_OPS_PER_RECOMPUTE};
use crate::layout::layout_metrics;
use crate::render::StatePanelLayout;
use crate::shared::{amplitude_qubits, now_seconds};
use crate::simulation_plan::{
    linearize_ops, validate_simulation_plan_capacity, SimulationPlanLimits,
};

struct CircuitFrameState {
    content_rect: Option<egui::Rect>,
    dragging_gate_id: Option<u32>,
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
        let colors = Colors::new();
        let content_height =
            self.circuit_content_height(self.layout_qubits(), screen_rect.height());

        // Decide whether wheel-over-the-panel should suppress the surrounding
        // ScrollArea's page-scroll. If pointer is on the state panel (or its
        // popover), route wheel to our aspect / zoom handlers instead.
        let pointer_over_state_panel = self.compute_state_panel_input_gate(ctx, screen_rect);

        let circuit_frame = self.show_circuit_scroll_area(
            ctx,
            ui,
            screen_rect,
            content_height,
            pointer_over_state_panel,
            &colors,
        );
        let state_frame = self.prepare_state_panel_frame(screen_rect);
        self.process_state_panel_interactions(ctx, ui, screen_rect, &state_frame);
        self.draw_frame_overlay(
            ctx,
            frame,
            screen_rect,
            &colors,
            &circuit_frame,
            &state_frame,
        );
    }

    fn show_circuit_scroll_area(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        screen_rect: egui::Rect,
        content_height: f32,
        pointer_over_state_panel: bool,
        colors: &Colors,
    ) -> CircuitFrameState {
        let mut frame_state = CircuitFrameState {
            content_rect: None,
            dragging_gate_id: None,
        };

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .scroll_source(egui::scroll_area::ScrollSource {
                drag: false,
                mouse_wheel: !pointer_over_state_panel,
                scroll_bar: true,
            })
            .show(ui, |ui| {
                let (rect, _response) = ui.allocate_exact_size(
                    egui::vec2(screen_rect.width(), content_height),
                    egui::Sense::click_and_drag(),
                );
                self.handle_input(rect, ctx, screen_rect);
                let content_changed = self.last_content_rect != Some(rect);
                self.last_content_rect = Some(rect);
                frame_state.content_rect = Some(rect);
                if content_changed {
                    ctx.request_repaint();
                }

                let metrics =
                    layout_metrics(rect.width(), self.layout_qubits(), self.min_circuit_slots());
                let painter = ui.painter_at(rect);
                let fast_drag = self.dragging.is_some();
                frame_state.dragging_gate_id = self.dragging.map(|drag| drag.id);
                self.draw_circuit(
                    &painter,
                    rect,
                    &metrics,
                    colors,
                    fast_drag,
                    frame_state.dragging_gate_id,
                    self.circuit_scroll_x,
                );
            });

        frame_state
    }

    fn prepare_state_panel_frame(&mut self, screen_rect: egui::Rect) -> StatePanelFrameState {
        // Resolve the state count / aspect / layout for this frame. While a gate
        // is mid-drag, an extra phantom qubit is added (`drag_state_count`) so
        // the layout doesn't reflow underneath the user during the drag.
        let base_state_count = self.state_count();
        let state_count = if self.dragging.is_some() {
            self.drag_state_count.unwrap_or(base_state_count)
        } else {
            base_state_count
        };
        let recompute = self.gpu_plan.needs_recompute_for(state_count);
        self.clamp_state_viewport_size();

        // Sync aspect_index with the current qubit count. While the user hasn't
        // customised, follow qni's per-qubit default; once customised, only clamp
        // to the valid [0, qubits] range so the choice stays sticky.
        let aspect_qubits = amplitude_qubits(state_count).clamp(1, MAX_QUBITS);
        if !self.state_panel.aspect_customized {
            self.state_panel.aspect_index = state_circle_default_aspect_index(aspect_qubits);
        } else {
            self.state_panel.aspect_index = self.state_panel.aspect_index.min(aspect_qubits);
        }
        let natural_circle_size =
            state_circle_layout(aspect_qubits, self.state_panel.aspect_index).size;
        let (min_zoom, max_zoom) = state_grid_zoom_limits(natural_circle_size);
        self.state_panel.grid_zoom = self.state_panel.grid_zoom.clamp(min_zoom, max_zoom);

        let layout = self.state_panel_layout(screen_rect, state_count);
        self.clamp_state_panel_offset(&layout, screen_rect);
        self.clamp_state_grid_offset(&layout);

        StatePanelFrameState {
            state_count,
            recompute,
            layout,
        }
    }

    fn process_state_panel_interactions(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        screen_rect: egui::Rect,
        state_frame: &StatePanelFrameState,
    ) {
        let state_rect = state_frame
            .layout
            .state_rect
            .translate(self.state_panel.offset);
        let handle_rect = egui::Rect::from_min_size(
            state_rect.min,
            egui::vec2(
                state_rect.width(),
                state_frame.layout.handle_height.max(6.0),
            ),
        );

        // Order matters: resize handles are registered last so they take
        // priority over the strip and viewport interacts at overlapping hits.
        self.process_state_panel_strip_drag(ui, &state_frame.layout, screen_rect, handle_rect);
        self.process_state_panel_viewport_pan_and_zoom(
            ctx,
            ui,
            &state_frame.layout,
            screen_rect,
            state_frame.state_count,
        );
        let dims_hit = QniApp::dims_hit_rect(ctx, &state_frame.layout, self.state_panel.offset);
        let aspect_qubits = amplitude_qubits(state_frame.state_count).clamp(1, MAX_QUBITS);
        self.process_aspect_dims(ctx, ui, aspect_qubits, dims_hit);
        self.process_aspect_popover(ctx, ui, aspect_qubits, dims_hit);
        self.process_resize_handles(ctx, ui, &state_frame.layout);
    }

    fn draw_frame_overlay(
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

    /// Refresh the simulation operation list + per-gate slot lookups when
    /// something changed (qubits added, gates rearranged, etc.). Returns the
    /// recompute flag to pass into rendering; false if we punted because the GPU
    /// target wasn't ready yet.
    fn process_gpu_recompute(
        &mut self,
        target_format: Option<eframe::wgpu::TextureFormat>,
        recompute: bool,
        state_count: usize,
        ctx: &egui::Context,
    ) -> bool {
        if target_format.is_some() {
            if recompute {
                self.gpu_plan.mark_clean_for(state_count);
                let qubits = self.state_qubits();
                // Hovered wins over breakpoint (live preview); `None` for both
                // = apply every column = final state.
                let step_limit = self.hovered_step.or(self.breakpoint_step);
                let sim_ops = linearize_ops(&self.placed_gates, qubits, step_limit);
                if let Err(error) = validate_simulation_plan_capacity(
                    &sim_ops,
                    SimulationPlanLimits {
                        max_ops_per_variant: MAX_OPS_PER_RECOMPUTE,
                        max_bloch_slots: MAX_BLOCH_SLOTS,
                        max_measurement_slots: MAX_MEASUREMENT_SLOTS,
                    },
                ) {
                    self.log_gpu_plan_capacity_error(&error.to_string());
                    self.gpu_plan.clear_ops();
                    return recompute;
                }
                self.gpu_plan.replace_ops(sim_ops);
            }
            recompute
        } else if recompute {
            ctx.request_repaint();
            false
        } else {
            false
        }
    }

    fn log_gpu_plan_capacity_error(&self, message: &str) {
        #[cfg(target_arch = "wasm32")]
        {
            let message = format!("qni-webgpu recompute skipped: {message}");
            web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(&message));
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = message;
        }
    }

    fn finish_update_frame(&mut self, ctx: &egui::Context, frame_start: f64) {
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

impl eframe::App for QniApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let frame_start = now_seconds();
        egui::CentralPanel::default().show(ctx, |ui| {
            self.show_update_central_panel(ctx, frame, ui);
        });
        self.finish_update_frame(ctx, frame_start);
    }
}
