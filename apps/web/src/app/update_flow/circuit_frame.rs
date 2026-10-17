use eframe::egui;

use super::CircuitFrameState;
use crate::app::QniApp;
use crate::colors::Colors;
use crate::constants::{TOOLBAR_HEIGHT, VERTICAL_CIRCUIT_GAP};
use crate::layout::PaletteCarousel;

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
        let mut frame_state = CircuitFrameState {
            content_rect: None,
            gpu_batch: None,
            recompute: false,
            dragging_gate_id: None,
            live_drag_gpu_overlay_ready: false,
            insert_preview_painted: false,
        };

        if self.orientation.is_vertical() {
            // The vertical layout pans its circuit along the time axis
            // itself; a page scroll would fight that pan for the drag.
            let rect = self.vertical_circuit_rect(screen_rect);
            ui.interact(
                rect,
                ui.id().with("vertical-circuit"),
                egui::Sense::click_and_drag(),
            );
            self.show_circuit_content(
                ctx,
                ui,
                rect,
                screen_rect,
                target_format,
                pointer_over_state_panel,
                colors,
                &mut frame_state,
            );
            return frame_state;
        }

        let content_height = self.circuit_content_height(self.layout_qubits(), screen_rect);
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
                self.show_circuit_content(
                    ctx,
                    ui,
                    rect,
                    screen_rect,
                    target_format,
                    pointer_over_state_panel,
                    colors,
                    &mut frame_state,
                );
            });

        frame_state
    }

    #[allow(clippy::too_many_arguments)]
    fn show_circuit_content(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        screen_rect: egui::Rect,
        target_format: Option<eframe::wgpu::TextureFormat>,
        pointer_over_state_panel: bool,
        colors: &Colors,
        frame_state: &mut CircuitFrameState,
    ) {
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

        let metrics = self.circuit_layout_metrics(rect);
        self.publish_layout_json(rect, screen_rect, &metrics);
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
            self.circuit_scroll_time,
        );
        frame_state.insert_preview_painted = insert_preview_painted;
        frame_state.gpu_batch = Some(gpu_batch);
        let pointer_pos = ctx.input(|input| input.pointer.latest_pos());
        let pointer_over_picker = self
            .picker_overlay_rect
            .is_some_and(|picker_rect| pointer_pos.is_some_and(|pos| picker_rect.contains(pos)));
        let pointer_over_vertical_panels =
            self.orientation.is_vertical() && pointer_pos.is_some_and(|pos| !rect.contains(pos));
        self.show_angle_input_overlay(
            ui,
            rect,
            &metrics,
            colors,
            frame_state.dragging_gate_id,
            self.circuit_scroll_time,
            pointer_over_state_panel || pointer_over_picker || pointer_over_vertical_panels,
        );
    }

    /// Circuit area of the vertical layout: between the state panel (or
    /// the toolbar) and the palette carousel.
    fn vertical_circuit_rect(&mut self, screen_rect: egui::Rect) -> egui::Rect {
        let top = if self.state_panel_visible() {
            self.prepare_state_panel_frame(screen_rect)
                .layout
                .state_rect
                .max
                .y
        } else {
            TOOLBAR_HEIGHT
        };
        let bottom = if self.palette_carousel_visible() {
            PaletteCarousel::new(screen_rect, self.palette.len())
                .rect()
                .min
                .y
        } else {
            screen_rect.max.y
        };
        egui::Rect::from_min_max(
            egui::pos2(screen_rect.min.x, top + VERTICAL_CIRCUIT_GAP),
            egui::pos2(screen_rect.max.x, bottom - VERTICAL_CIRCUIT_GAP),
        )
    }
}

impl QniApp {
    /// Publishes screen geometry of the layout for browser tests: the
    /// orientation, the circuit area, each (column, wire) cell centre, and
    /// the vertical layout's palette carousel. Geometry only; no state
    /// values.
    #[cfg(all(target_arch = "wasm32", debug_assertions))]
    fn publish_layout_json(
        &self,
        rect: egui::Rect,
        screen_rect: egui::Rect,
        metrics: &crate::layout::LayoutMetrics,
    ) {
        let point = |pos: egui::Pos2| format!("{{\"x\":{:.3},\"y\":{:.3}}}", pos.x, pos.y);
        let rect_json = |rect: egui::Rect| {
            format!(
                "{{\"left\":{:.3},\"top\":{:.3},\"right\":{:.3},\"bottom\":{:.3}}}",
                rect.left(),
                rect.top(),
                rect.right(),
                rect.bottom()
            )
        };
        let projection = self.circuit_projection(rect, self.circuit_scroll_time);
        let cells = metrics
            .slot_centers
            .iter()
            .map(|&x| {
                let wires = metrics
                    .line_ys
                    .iter()
                    .map(|&y| point(projection.to_screen(egui::pos2(x, y))))
                    .collect::<Vec<_>>()
                    .join(",");
                format!("[{wires}]")
            })
            .collect::<Vec<_>>()
            .join(",");
        let carousel = if self.orientation.is_vertical() && self.palette_carousel_visible() {
            let carousel = PaletteCarousel::new(screen_rect, self.palette.len());
            let slots = (0..crate::constants::CAROUSEL_GATES_PER_PAGE)
                .map(|slot| rect_json(carousel.slot_rect(slot)))
                .collect::<Vec<_>>()
                .join(",");
            let dots = (0..carousel.page_count())
                .map(|page| point(carousel.dot_center(page)))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"rect\":{},\"page\":{},\"pageCount\":{},\"slots\":[{slots}],\"prev\":{},\"next\":{},\"dots\":[{dots}]}}",
                rect_json(carousel.rect()),
                carousel.clamp_page(self.palette_page),
                carousel.page_count(),
                rect_json(carousel.prev_rect()),
                rect_json(carousel.next_rect()),
            )
        } else {
            "null".to_owned()
        };
        let orientation = if self.orientation.is_vertical() {
            "vertical"
        } else {
            "horizontal"
        };
        let json = format!(
            "{{\"orientation\":\"{orientation}\",\"circuitRect\":{},\"cells\":[{cells}],\"carousel\":{carousel}}}",
            rect_json(rect),
        );
        crate::test_hooks::set_window_value(
            crate::test_hooks::QNI_LAYOUT_JSON,
            &wasm_bindgen::JsValue::from_str(&json),
        );
    }

    #[cfg(any(not(target_arch = "wasm32"), not(debug_assertions)))]
    fn publish_layout_json(
        &self,
        _rect: egui::Rect,
        _screen_rect: egui::Rect,
        _metrics: &crate::layout::LayoutMetrics,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertical_circuit_area_stays_put_while_a_drag_grows_the_state_panel() {
        let ctx = egui::Context::default();
        let mut app = QniApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()));
        app.load_circuit_json_into_editor(r#"{"cols":[["H"],["•","X"]]}"#, &ctx);
        app.orientation = crate::layout::CircuitOrientation::Vertical;
        let screen = egui::Rect::from_min_max(egui::pos2(8.0, 8.0), egui::pos2(382.0, 836.0));
        let before = app.vertical_circuit_rect(screen);
        app.dragging = Some(crate::app::DragState {
            id: app.placed_gates[0].id,
            preview_pos: app.placed_gates[0].committed_pos(),
            offset: egui::Vec2::ZERO,
            original_column: None,
            click_copy: None,
        });
        app.drag_state_count = Some(8);

        assert_eq!(app.vertical_circuit_rect(screen), before);
    }

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
