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
}
