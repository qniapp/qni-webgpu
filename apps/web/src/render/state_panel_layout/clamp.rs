use eframe::egui;

use super::StatePanelLayout;
use crate::app::QniApp;

impl QniApp {
    pub(crate) fn clamp_state_panel_offset(&mut self, layout: &StatePanelLayout, rect: egui::Rect) {
        // The whole panel may extend past the screen edges (especially for
        // 16-qubit grids that are wider than the canvas), but the drag
        // handle must stay reachable — keep at least `MIN_VISIBLE` pixels
        // of it inside `rect` on both axes.
        const MIN_VISIBLE: f32 = 40.0;

        if self.orientation.is_vertical() {
            self.state_panel.offset = egui::Vec2::ZERO;
            return;
        }

        if matches!(self.mode, crate::app::AppMode::Embed { .. }) {
            let min = egui::vec2(
                rect.left() - layout.state_rect.left(),
                self.circuit_bottom_y(rect) + crate::constants::EMBED_STATE_PANEL_CIRCUIT_GAP
                    - layout.state_rect.top(),
            );
            let max = egui::vec2(
                rect.right() - layout.state_rect.right(),
                rect.bottom() - layout.state_rect.bottom(),
            );
            self.state_panel.offset.x = self.state_panel.offset.x.clamp(min.x.min(max.x), max.x);
            self.state_panel.offset.y = self.state_panel.offset.y.clamp(min.y, max.y.max(min.y));
            return;
        }

        let panel_w = layout.state_rect.width();
        let handle_h = layout.handle_height;

        let min_x = rect.min.x + MIN_VISIBLE - panel_w;
        let max_x = rect.max.x - MIN_VISIBLE;
        let min_y = rect.min.y + MIN_VISIBLE - handle_h;
        let max_y = rect.max.y - MIN_VISIBLE;

        let base_min = layout.state_rect.min;
        let min_offset_x = min_x - base_min.x;
        let max_offset_x = max_x - base_min.x;
        let min_offset_y = min_y - base_min.y;
        let max_offset_y = max_y - base_min.y;

        self.state_panel.offset.x = if max_offset_x < min_offset_x {
            min_offset_x
        } else {
            self.state_panel.offset.x.clamp(min_offset_x, max_offset_x)
        };
        self.state_panel.offset.y = if max_offset_y < min_offset_y {
            min_offset_y
        } else {
            self.state_panel.offset.y.clamp(min_offset_y, max_offset_y)
        };
    }

    /// Keep `state_grid_offset` inside the range that lets `grid_origin`
    /// produce a non-flickering value: when the grid fits on an axis, pan can
    /// move it within the centred slack; when it overflows, pan is
    /// `[viewport - grid, 0]`. Called every frame after layout computation so
    /// qubit-count / aspect changes don't leave a stale huge offset around.
    pub(crate) fn clamp_state_grid_offset(&mut self, layout: &StatePanelLayout) {
        let viewport = layout.viewport_rect.translate(self.state_panel.offset);
        let grid = layout.grid_size;
        self.state_panel.grid_offset.x =
            clamp_grid_axis_offset(self.state_panel.grid_offset.x, viewport.width(), grid.x);
        self.state_panel.grid_offset.y =
            clamp_grid_axis_offset(self.state_panel.grid_offset.y, viewport.height(), grid.y);
    }
}

fn clamp_grid_axis_offset(offset: f32, viewport_size: f32, grid_size: f32) -> f32 {
    if grid_size <= viewport_size {
        let slack = (viewport_size - grid_size) * 0.5;
        offset.clamp(-slack, slack)
    } else {
        offset.clamp(viewport_size - grid_size, 0.0)
    }
}
