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

        let panel_w = layout.state_rect.width();
        let handle_h = layout.handle_height;

        // Horizontal: panel right edge ≥ rect.min.x + MIN_VISIBLE  (left clip)
        //             panel left  edge ≤ rect.max.x − MIN_VISIBLE  (right clip)
        let min_x = rect.min.x + MIN_VISIBLE - panel_w;
        let max_x = rect.max.x - MIN_VISIBLE;
        // Vertical: handle bottom ≥ rect.min.y + MIN_VISIBLE   (top clip)
        //           handle top    ≤ rect.max.y − MIN_VISIBLE   (bottom clip)
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
    /// produce a non-flickering value: 0 when the grid fits on an axis,
    /// `[viewport - grid, 0]` when it overflows. Called every frame after
    /// the layout is computed so qubit-count changes don't leave a stale
    /// (possibly huge) pan offset around.
    pub(crate) fn clamp_state_grid_offset(&mut self, layout: &StatePanelLayout) {
        let viewport = layout.viewport_rect.translate(self.state_panel.offset);
        let grid = layout.grid_size;
        self.state_panel.grid_offset.x = if grid.x <= viewport.width() {
            0.0
        } else {
            self.state_panel
                .grid_offset
                .x
                .clamp(viewport.width() - grid.x, 0.0)
        };
        self.state_panel.grid_offset.y = if grid.y <= viewport.height() {
            0.0
        } else {
            self.state_panel
                .grid_offset
                .y
                .clamp(viewport.height() - grid.y, 0.0)
        };
    }
}
