use eframe::egui;

use crate::app::QniApp;

impl QniApp {
    /// Aspect popover (D 案) layout. Anchored to the bottom-right corner
    /// of the dimensions text, opening downward. Each row corresponds to
    /// one `aspect_index ∈ [0, qubits]` choice. Returns the popover rect
    /// (for outside-click detection) plus a Vec of per-row rects (for
    /// click-to-pick interaction and matching draw geometry).
    pub(crate) fn aspect_popover_layout(
        dims_rect: egui::Rect,
        qubits: usize,
    ) -> (egui::Rect, Vec<egui::Rect>) {
        const ROW_HEIGHT: f32 = 22.0;
        const PADDING: f32 = 8.0;
        const WIDTH: f32 = 240.0;
        const MAX_HEIGHT: f32 = 420.0;
        let n_rows = qubits + 1;
        let content_height = n_rows as f32 * ROW_HEIGHT;
        let total_height = (content_height + PADDING * 2.0).min(MAX_HEIGHT);
        let rect = egui::Rect::from_min_size(
            egui::pos2(dims_rect.max.x - WIDTH, dims_rect.max.y + 2.0),
            egui::vec2(WIDTH, total_height),
        );
        let mut rows = Vec::with_capacity(n_rows);
        for i in 0..n_rows {
            let y = rect.min.y + PADDING + (i as f32 * ROW_HEIGHT);
            rows.push(egui::Rect::from_min_size(
                egui::pos2(rect.min.x + PADDING, y),
                egui::vec2(WIDTH - PADDING * 2.0, ROW_HEIGHT - 2.0),
            ));
        }
        (rect, rows)
    }
}
