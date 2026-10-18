use eframe::egui;

use super::DragController;
use crate::app::QniApp;
use crate::layout::LayoutMetrics;

impl DragController {
    pub(in crate::app) fn update_circuit_scroll(
        app: &mut QniApp,
        ctx: &egui::Context,
        content_rect: egui::Rect,
        screen_pos: Option<egui::Pos2>,
        metrics: &LayoutMetrics,
    ) {
        // Horizontal scroll: trackpad horizontal swipes come through
        // as `smooth_scroll_delta.x`; desktop mice typically only
        // produce a `delta.y`, so we treat shift+wheel-y as wheel-x
        // when the cursor is inside the circuit area. Scroll right
        // (positive delta) → reveal trailing gates → `scroll_x`
        // grows. Always clamp to `CircuitGutters::max_scroll` so the
        // wire end stops one right gutter before the canvas edge.
        let cursor_in_circuit = screen_pos.is_some_and(|p| content_rect.contains(p));
        if cursor_in_circuit {
            let (raw_dx, raw_dy, shift) = ctx.input(|i| {
                (
                    i.smooth_scroll_delta.x,
                    i.smooth_scroll_delta.y,
                    i.modifiers.shift,
                )
            });
            // The vertical layout's time axis runs down the screen, so a
            // plain wheel scrolls it.
            let dx = if app.orientation.is_vertical() {
                raw_dy
            } else if raw_dx.abs() > raw_dy.abs() || !shift {
                raw_dx
            } else {
                raw_dy
            };
            if dx != 0.0 {
                let max_scroll = app.max_circuit_scroll(content_rect, metrics);
                app.circuit_scroll_time = (app.circuit_scroll_time - dx).clamp(0.0, max_scroll);
                ctx.request_repaint();
            }
        }
        // After the scroll update, force a clamp so newly-loaded
        // circuits or window resizes never leave us scrolled past the
        // current content extent.
        let max_scroll = app.max_circuit_scroll(content_rect, metrics);
        if app.circuit_scroll_time > max_scroll {
            app.circuit_scroll_time = max_scroll;
        }
    }
}

impl QniApp {
    /// Largest time scroll for `content_rect`; `0` when the circuit fits.
    pub(in crate::app) fn max_circuit_scroll(
        &self,
        content_rect: egui::Rect,
        metrics: &LayoutMetrics,
    ) -> f32 {
        self.circuit_gutters(content_rect)
            .max_scroll(metrics, self.circuit_time_length(content_rect))
    }
}
