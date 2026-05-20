use eframe::egui;

use crate::app::QniApp;

impl QniApp {
    /// Aspect popover layout. Anchored just below the dimensions trigger and
    /// expanded left from the state-vector panel's right edge so the trigger
    /// remains clickable for close/toggle while the popover right edge still
    /// matches the panel right edge. Chrome and rows mirror the Circuit picker.
    pub(crate) fn aspect_popover_layout(
        panel_rect: egui::Rect,
        dims_rect: egui::Rect,
        qubits: usize,
        bounds: egui::Rect,
    ) -> (egui::Rect, Vec<egui::Rect>) {
        const ROW_HEIGHT: f32 = 36.0; // h-9 / picker item height.
        const PADDING: f32 = 6.0; // p-1.5 = 6px.
        const WIDTH: f32 = 240.0; // match Circuit picker dropdown.
        const TRIGGER_GAP: f32 = 2.0;
        let n_rows = qubits + 1;
        let total_height = n_rows as f32 * ROW_HEIGHT + PADDING * 2.0;
        let down_top = dims_rect.bottom() + TRIGGER_GAP;
        let down_bottom = down_top + total_height;
        let up_top = dims_rect.top() - TRIGGER_GAP - total_height;
        let top = if down_bottom <= bounds.bottom() {
            down_top
        } else if up_top >= bounds.top() {
            up_top
        } else {
            (bounds.bottom() - total_height).max(bounds.top())
        };
        let rect = egui::Rect::from_min_size(
            egui::pos2(panel_rect.right() - WIDTH, top),
            egui::vec2(WIDTH, total_height),
        );
        let mut rows = Vec::with_capacity(n_rows);
        for i in 0..n_rows {
            let y = rect.min.y + PADDING + (i as f32 * ROW_HEIGHT);
            rows.push(egui::Rect::from_min_size(
                egui::pos2(rect.min.x + PADDING, y),
                egui::vec2(WIDTH - PADDING * 2.0, ROW_HEIGHT),
            ));
        }
        (rect, rows)
    }
}
