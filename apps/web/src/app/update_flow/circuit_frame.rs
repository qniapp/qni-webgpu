use eframe::egui;

use super::CircuitFrameState;
use crate::app::QniApp;
use crate::colors::Colors;
use crate::layout::layout_metrics;

impl QniApp {
    pub(super) fn show_circuit_scroll_area(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        screen_rect: egui::Rect,
        target_format: Option<eframe::wgpu::TextureFormat>,
        pointer_over_state_panel: bool,
        colors: &Colors,
    ) -> CircuitFrameState {
        let content_height = self.circuit_content_height(self.layout_qubits(), screen_rect);
        let mut frame_state = CircuitFrameState {
            content_rect: None,
            gpu_batch: None,
            recompute: false,
            dragging_gate_id: None,
            live_drag_gpu_overlay_ready: false,
            insert_preview_painted: false,
        };

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .scroll_source(egui::scroll_area::ScrollSource {
                drag: egui::scroll_area::DragScroll::Never,
                mouse_wheel: !pointer_over_state_panel,
                scroll_bar: true,
            })
            .show(ui, |ui| {
                let (rect, _response) = ui.allocate_exact_size(
                    egui::vec2(screen_rect.width(), content_height),
                    egui::Sense::click_and_drag(),
                );
                self.handle_input(rect, ctx, screen_rect, pointer_over_state_panel);
                let state_frame = self.prepare_state_panel_frame(screen_rect);
                frame_state.recompute = self.process_gpu_recompute(
                    target_format,
                    state_frame.recompute,
                    state_frame.state_count,
                    ctx,
                );
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
                frame_state.live_drag_gpu_overlay_ready = frame_state
                    .dragging_gate_id
                    .is_some_and(|gate_id| self.live_drag_gpu_overlay_ready(gate_id));
                let (insert_preview_painted, gpu_batch) = self.draw_circuit(
                    &painter,
                    rect,
                    &metrics,
                    colors,
                    fast_drag,
                    frame_state.dragging_gate_id,
                    self.circuit_scroll_x,
                );
                frame_state.insert_preview_painted = insert_preview_painted;
                frame_state.gpu_batch = Some(gpu_batch);
                let pointer_pos = ctx.input(|input| input.pointer.latest_pos());
                let pointer_over_picker = self.picker_overlay_rect.is_some_and(|picker_rect| {
                    pointer_pos.is_some_and(|pos| picker_rect.contains(pos))
                });
                self.show_angle_input_overlay(
                    ui,
                    rect,
                    &metrics,
                    colors,
                    frame_state.dragging_gate_id,
                    self.circuit_scroll_x,
                    pointer_over_state_panel || pointer_over_picker,
                );
            });

        frame_state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circuit_batch_uses_the_refreshed_gpu_plan_in_the_same_frame() {
        let ctx = egui::Context::default();
        let mut app = QniApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()));
        app.load_circuit_json_into_editor(r#"{"cols":[["Measure>a"],["X<a"],["Bloch"]]}"#, &ctx);
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0));
        let colors = app.colors();
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(rect),
                ..Default::default()
            },
            |root_ui| {
                egui::CentralPanel::default().show(root_ui, |ui| {
                    app.show_circuit_scroll_area(
                        &ctx,
                        ui,
                        rect,
                        Some(eframe::wgpu::TextureFormat::Rgba8Unorm),
                        false,
                        &colors,
                    );
                });
            },
        );
        let callbacks = output.shapes.into_iter().filter(|clipped| {
            matches!(&clipped.shape, egui::Shape::Callback(callback) if callback.rect.width() > 1000.0)
        }).count();
        assert_eq!(callbacks, 3);
    }
}
