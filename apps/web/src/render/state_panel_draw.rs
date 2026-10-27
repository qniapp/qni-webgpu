mod gpu_callback;
mod header;
mod overlays;
mod panel;

use eframe::egui;
use eframe::wgpu;

use super::state_panel_layout::StatePanelLayout;
use crate::app::QniApp;
use crate::colors::Colors;

impl QniApp {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_state_vector(
        &mut self,
        painter: &egui::Painter,
        colors: &Colors,
        layout: &StatePanelLayout,
        offset: egui::Vec2,
        handle_height: f32,
        screen_rect: egui::Rect,
        target_format: Option<wgpu::TextureFormat>,
    ) -> egui::Rect {
        let state_rect = layout.state_rect.translate(offset);
        let viewport_rect = layout.viewport_rect.translate(offset);
        let grid_origin = Self::grid_origin(layout, offset, self.state_panel.grid_offset);

        panel::paint_panel_background(painter, colors, state_rect);
        let handle_rect =
            header::paint_header_strip(painter, colors, layout, state_rect, handle_height);
        if let Some(message) = self.gpu_plan.capacity_error() {
            panel::paint_capacity_error(painter, colors, viewport_rect, message);
        } else {
            gpu_callback::paint_state_vector_gpu(
                self,
                painter,
                colors,
                layout,
                viewport_rect,
                grid_origin,
                screen_rect,
                target_format,
            );
        }
        overlays::paint_state_panel_overlays(
            self,
            painter,
            colors,
            layout,
            offset,
            state_rect,
            viewport_rect,
            grid_origin,
            screen_rect,
        );

        handle_rect
    }
}
