use eframe::egui;

use super::StatePanelLayout;
use crate::app::QniApp;
use crate::constants::{state_circle_layout, STATE_CIRCLE_BOTTOM_MARGIN, STATE_HANDLE_HEIGHT};
use crate::shared::amplitude_qubits;

impl QniApp {
    pub(crate) fn state_panel_layout(
        &self,
        rect: egui::Rect,
        state_count: usize,
    ) -> StatePanelLayout {
        let state_count = state_count.max(1);
        let qubits = amplitude_qubits(state_count);

        // Cell size + line width follow qni's per-qubit-count table; the
        // (cols, rows) split is parameterised by `self.state_panel.aspect_index` so
        // the user can change the layout aspect at runtime. qni's
        // reference uses gap == stroke (cells touch); we add 1 px so
        // adjacent stroke rings don't share a pixel boundary at dist ==
        // outer. Without this slack the GPU-side single-cell render
        // gives 50 % alpha at the boundary (symmetric smoothstep midpoint
        // is exactly 0.5), visibly fading the outline. The 1-px seam is
        // barely perceptible at typical zoom and lets us keep V-sync at
        // 11+ qubits without paying for 2x2 cell sampling in the
        // fragment shader.
        let qni = state_circle_layout(qubits, self.state_panel.aspect_index);
        let columns = qni.cols;
        let rows = qni.rows;
        // Zoom scales every length-y thing in the grid uniformly so cells
        // grow / shrink together. Stroke has a 0.5 px floor so very-zoomed-
        // out cells still get a visible outline rather than collapsing into
        // pure fill.
        let zoom = self.state_panel.grid_zoom;
        let size = qni.size * zoom;
        let stroke = (qni.line_width * zoom).max(0.5);
        let gap = (qni.line_width + 1.0) * zoom;

        let total_width = size * columns as f32 + gap * (columns.saturating_sub(1)) as f32;
        let total_height = size * rows as f32 + gap * (rows.saturating_sub(1)) as f32;
        let radius = size * 0.5;
        let inner_radius = (radius - stroke * 0.5).max(0.0);

        // qni-style header strip (G-2): fixed-height zinc-100 bar showing
        // qubit count + grid dims. Drag-to-move is the only interaction
        // attached to the strip for now (resize handles are TBD).
        let handle_height = STATE_HANDLE_HEIGHT;

        // Make sure the panel is wide enough that the strip's left/right
        // labels never overlap. Geist Mono at text-sm (14 px) is ≈ 9 px /
        // glyph; budget a bit extra for the multiplication sign.
        const STRIP_CHAR_WIDTH: f32 = 9.0;
        // spacing-3 (12px) padding, spacing-4 (16px) gap between labels.
        const STRIP_PADDING_X: f32 = 12.0;
        const STRIP_LABEL_GAP: f32 = 16.0;
        let qubits_label = if qubits == 1 { "qubit" } else { "qubits" };
        let states_label = if state_count == 1 { "state" } else { "states" };
        let qubits_chars = format!("{qubits} {qubits_label}").chars().count();
        // "+ 2" reserves room for the " ▾" suffix on the right text that
        // signals the aspect popover is openable.
        let states_chars = format!("{columns} × {rows} = {state_count} {states_label}")
            .chars()
            .count()
            + 2;
        let strip_min_width = (qubits_chars + states_chars) as f32 * STRIP_CHAR_WIDTH
            + STRIP_PADDING_X * 2.0
            + STRIP_LABEL_GAP;

        // Panel size is user-controlled (resize via the corner L-handles).
        // Strip-text minimum is the only thing that can force `panel_width`
        // above the user's choice — practically a no-op for ≤16 qubits
        // since min viewport width already covers the widest label.
        let panel_width = self.state_panel.viewport_size.x.max(strip_min_width);
        let panel_height = self.state_panel.viewport_size.y + handle_height;
        let panel_min_x = rect.width() / 2.0 - panel_width / 2.0;
        let panel_min_y = rect.height() - STATE_CIRCLE_BOTTOM_MARGIN - panel_height;
        let state_rect = egui::Rect::from_min_size(
            rect.min + egui::vec2(panel_min_x, panel_min_y),
            egui::vec2(panel_width, panel_height),
        );
        let viewport_rect = egui::Rect::from_min_max(
            state_rect.min + egui::vec2(0.0, handle_height),
            state_rect.max,
        );

        StatePanelLayout {
            state_count,
            qubits,
            columns,
            size,
            gap,
            radius,
            stroke,
            inner_radius,
            grid_size: egui::vec2(total_width, total_height),
            viewport_rect,
            state_rect,
            handle_height,
        }
    }

    /// Where the circle grid's top-left corner should render given the panel
    /// layout and the user's pan offset. A fitting grid starts centred, but
    /// pan still applies within the available slack so wheel zoom can keep
    /// the cursor anchor fixed instead of always expanding from the centre.
    /// Once the grid overflows, pan is clamped so its edges stay attached to
    /// the viewport edges (no empty bands beyond the grid).
    pub(crate) fn grid_origin(
        layout: &StatePanelLayout,
        viewport_offset: egui::Vec2,
        pan: egui::Vec2,
    ) -> egui::Pos2 {
        let viewport = layout.viewport_rect.translate(viewport_offset);
        let grid = layout.grid_size;
        egui::pos2(
            grid_axis_origin(viewport.min.x, viewport.width(), grid.x, pan.x),
            grid_axis_origin(viewport.min.y, viewport.height(), grid.y, pan.y),
        )
    }

    /// Convert a desired grid top-left origin into the pan value that
    /// `grid_origin` expects for a given layout. Used by cursor-anchored zoom
    /// after the zoomed layout is known, avoiding base-origin drift when the
    /// grid transitions between "fits and centred" and "overflows" modes.
    pub(crate) fn grid_offset_for_origin(
        layout: &StatePanelLayout,
        viewport_offset: egui::Vec2,
        origin: egui::Pos2,
    ) -> egui::Vec2 {
        let viewport = layout.viewport_rect.translate(viewport_offset);
        let grid = layout.grid_size;
        egui::vec2(
            grid_axis_pan_for_origin(viewport.min.x, viewport.width(), grid.x, origin.x),
            grid_axis_pan_for_origin(viewport.min.y, viewport.height(), grid.y, origin.y),
        )
    }
}

fn grid_axis_origin(viewport_min: f32, viewport_size: f32, grid_size: f32, pan: f32) -> f32 {
    if grid_size <= viewport_size {
        let slack = (viewport_size - grid_size) * 0.5;
        (viewport_min + slack + pan).clamp(viewport_min, viewport_min + slack * 2.0)
    } else {
        (viewport_min + pan).clamp(viewport_min + viewport_size - grid_size, viewport_min)
    }
}

fn grid_axis_pan_for_origin(
    viewport_min: f32,
    viewport_size: f32,
    grid_size: f32,
    origin: f32,
) -> f32 {
    if grid_size <= viewport_size {
        let slack = (viewport_size - grid_size) * 0.5;
        origin - (viewport_min + slack)
    } else {
        origin - viewport_min
    }
}
