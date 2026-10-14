use eframe::egui;

use super::StatePanelLayout;
use crate::app::{AppMode, QniApp};
use crate::constants::{
    state_circle_layout, EMBED_STATE_PANEL_CIRCUIT_GAP, EMBED_STATE_PANEL_TIGHT_BOTTOM_MARGIN,
    STATE_CIRCLE_BOTTOM_MARGIN, STATE_HANDLE_HEIGHT, STATE_VIEWPORT_MIN_HEIGHT,
};
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
        let embed = matches!(self.mode, AppMode::Embed { .. });
        // Fit the initial state vector inside narrow embeds. User zoom is
        // relative to this fit, so all states are visible before interaction.
        let natural_width =
            qni.size * qni.cols as f32 + (qni.line_width + 1.0) * qni.cols.saturating_sub(1) as f32;
        let natural_height =
            qni.size * qni.rows as f32 + (qni.line_width + 1.0) * qni.rows.saturating_sub(1) as f32;
        // qni-style header strip (G-2): fixed-height zinc-100 bar showing
        // qubit count + grid dims. Drag-to-move is the only interaction
        // attached to the strip for now (resize handles are TBD).
        let handle_height = STATE_HANDLE_HEIGHT;

        // Make sure the panel is wide enough that the strip's left/right
        // labels never overlap. Geist Mono at 14px is ≈ 9 px /
        // glyph; budget a bit extra for the multiplication sign.
        const STRIP_CHAR_WIDTH: f32 = 9.0;
        // 12px padding, 16px gap between labels.
        const STRIP_PADDING_X: f32 = crate::tailwind::SPACING_3;
        const STRIP_LABEL_GAP: f32 = crate::tailwind::SPACING_4;
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
        let viewport_width = if embed {
            // Narrow embeds (phone-width tutorial pages) keep the panel inside
            // the canvas with the same 16px side margin.
            self.state_panel
                .viewport_size
                .x
                .min((rect.width() - 2.0 * EMBED_STATE_PANEL_TIGHT_BOTTOM_MARGIN).max(1.0))
        } else {
            self.state_panel.viewport_size.x
        };
        let panel_width = if embed {
            // Keep narrow tutorial panels inside the canvas. The header omits
            // the redundant qubit label when both labels no longer fit.
            viewport_width
        } else {
            viewport_width.max(strip_min_width)
        };
        let (panel_min_y, viewport_height) = if embed {
            embed_panel_vertical(
                rect.height(),
                self.circuit_bottom_y(rect) - rect.min.y,
                self.state_panel.viewport_size.y,
            )
        } else {
            let viewport_height = self.state_panel.viewport_size.y;
            (
                rect.height() - STATE_CIRCLE_BOTTOM_MARGIN - handle_height - viewport_height,
                viewport_height,
            )
        };
        let fit = if embed {
            (panel_width.max(1.0) / natural_width)
                .min(viewport_height / natural_height)
                .min(1.0)
        } else {
            1.0
        };
        let zoom = self.state_panel.grid_zoom * fit;
        let size = qni.size * zoom;
        let stroke = (qni.line_width * zoom).max(0.5);
        let gap = (qni.line_width + 1.0) * zoom;
        let total_width = size * columns as f32 + gap * columns.saturating_sub(1) as f32;
        let total_height = size * rows as f32 + gap * rows.saturating_sub(1) as f32;
        let radius = size * 0.5;
        let inner_radius = (radius - stroke * 0.5).max(0.0);
        let panel_height = viewport_height + handle_height;
        let panel_min_x = rect.width() / 2.0 - panel_width / 2.0;
        let state_rect = egui::Rect::from_min_size(
            rect.min + egui::vec2(panel_min_x, panel_min_y),
            egui::vec2(panel_width, panel_height),
        );
        let viewport_rect = egui::Rect::from_min_max(
            state_rect.min + egui::vec2(0.0, handle_height),
            state_rect.max,
        );

        StatePanelLayout {
            show_restart_button: self.on_restart.is_some(),
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

/// Vertical placement `(panel_min_y, viewport_height)` of an embed's state
/// panel, relative to the canvas top. Small embeds must never hide gates:
/// the panel keeps the standalone bottom-anchored spot when it clears the
/// circuit, otherwise it sits right below the circuit and its viewport
/// shrinks to fit (never below the minimum, clipping at the canvas edge).
pub(crate) fn embed_panel_vertical(
    canvas_height: f32,
    circuit_bottom: f32,
    viewport_height: f32,
) -> (f32, f32) {
    let top_limit = circuit_bottom + EMBED_STATE_PANEL_CIRCUIT_GAP;
    let anchored_min_y =
        canvas_height - STATE_CIRCLE_BOTTOM_MARGIN - STATE_HANDLE_HEIGHT - viewport_height;
    if anchored_min_y >= top_limit {
        return (anchored_min_y, viewport_height);
    }
    let available =
        canvas_height - EMBED_STATE_PANEL_TIGHT_BOTTOM_MARGIN - STATE_HANDLE_HEIGHT - top_limit;
    (
        top_limit,
        available.clamp(
            STATE_VIEWPORT_MIN_HEIGHT,
            viewport_height.max(STATE_VIEWPORT_MIN_HEIGHT),
        ),
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    const CANVAS_HEIGHT: f32 = 560.0;

    #[test]
    fn embed_panel_keeps_bottom_anchor_when_clear_of_circuit() {
        let (min_y, _) = embed_panel_vertical(CANVAS_HEIGHT, 100.0, 160.0);
        assert_eq!(
            min_y,
            CANVAS_HEIGHT - STATE_CIRCLE_BOTTOM_MARGIN - STATE_HANDLE_HEIGHT - 160.0
        );
    }

    #[test]
    fn embed_panel_moves_below_circuit_when_anchor_would_cover_it() {
        let (min_y, _) = embed_panel_vertical(CANVAS_HEIGHT, 348.0, 160.0);
        assert_eq!(min_y, 348.0 + EMBED_STATE_PANEL_CIRCUIT_GAP);
    }

    #[test]
    fn embed_panel_viewport_shrinks_to_fit_below_circuit() {
        let (_, viewport) = embed_panel_vertical(CANVAS_HEIGHT, 348.0, 160.0);
        assert_eq!(
            viewport,
            CANVAS_HEIGHT
                - EMBED_STATE_PANEL_TIGHT_BOTTOM_MARGIN
                - STATE_HANDLE_HEIGHT
                - (348.0 + EMBED_STATE_PANEL_CIRCUIT_GAP)
        );
    }

    #[test]
    fn embed_panel_viewport_never_shrinks_below_minimum() {
        let (_, viewport) = embed_panel_vertical(CANVAS_HEIGHT, 520.0, 160.0);
        assert_eq!(viewport, STATE_VIEWPORT_MIN_HEIGHT);
    }

    fn embed_app(json: &str, palette: Option<&[&str]>) -> QniApp {
        let ctx = egui::Context::default();
        let startup = crate::app::EmbedStartup::parse(json, true, palette, None).unwrap();
        QniApp::new_with_startup(&eframe::CreationContext::_new_kittest(ctx), Some(startup))
    }

    fn small_embed_rect() -> egui::Rect {
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(960.0, CANVAS_HEIGHT))
    }

    #[test]
    fn small_full_palette_embed_panel_clears_cnot_target() {
        let app = embed_app(r#"{"cols":[["|0>","|0>"],["H"],["•","X"]]}"#, None);
        let rect = small_embed_rect();
        let layout = app.state_panel_layout(rect, 4);
        assert!(layout.state_rect.min.y >= app.circuit_bottom_y(rect));
    }

    #[test]
    fn small_restricted_palette_embed_panel_clears_cnot_target() {
        let app = embed_app(
            r#"{"cols":[["|0>","|0>"],["H"],["•","X"]]}"#,
            Some(&["H", "•", "X"]),
        );
        let rect = small_embed_rect();
        let layout = app.state_panel_layout(rect, 4);
        assert!(layout.state_rect.min.y >= app.circuit_bottom_y(rect));
    }

    #[test]
    fn narrow_embed_panel_stays_inside_canvas() {
        let app = embed_app(r#"{"cols":[["|0>"]]}"#, Some(&["H", "X"]));
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(354.0, 592.0));
        let layout = app.state_panel_layout(rect, 2);
        assert!(rect.contains_rect(layout.state_rect));
    }

    #[test]
    fn initial_grid_fits_a_shrunk_explicit_height() {
        let mut app = embed_app(r#"{"cols":[["H"]]}"#, None);
        app.state_panel.aspect_index = 4;
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(320.0, 400.0));
        let layout = app.state_panel_layout(rect, 256);
        assert!(
            layout.grid_size.x <= layout.viewport_rect.width() + 0.01
                && layout.grid_size.y <= layout.viewport_rect.height() + 0.01
        );
    }

    #[test]
    fn fitted_grid_can_zoom_to_full_size_cells() {
        let mut app = embed_app(r#"{"cols":[["H"]]}"#, None);
        app.state_panel.aspect_index = 8;
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(320.0, 600.0));
        let layout = app.state_panel_layout(rect, 65536);
        let (_, maximum) = layout.zoom_limits(app.state_panel.grid_zoom);
        app.state_panel.grid_zoom = maximum;
        assert!((app.state_panel_layout(rect, 65536).size - 256.0).abs() < 0.01);
    }

    #[test]
    fn insufficient_explicit_height_never_moves_panel_over_wires() {
        let mut app = embed_app(r#"{"cols":[["H","H","H","H","H","H"]]}"#, None);
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(320.0, 400.0));
        let layout = app.state_panel_layout(rect, 64);
        app.clamp_state_panel_offset(&layout, rect);
        assert!(
            layout.state_rect.translate(app.state_panel.offset).top()
                >= app.circuit_bottom_y(rect) + EMBED_STATE_PANEL_CIRCUIT_GAP
        );
    }

    #[test]
    fn resize_in_shrunk_embed_starts_from_visible_viewport() {
        let mut app = embed_app(r#"{"cols":[["|0>","|0>"],["H"],["•","X"]]}"#, None);
        let layout = app.state_panel_layout(small_embed_rect(), 4);
        let pointer = layout.state_rect.right_bottom();
        app.begin_resize_drag(
            crate::app::ResizeCorner::BottomRight,
            pointer,
            layout.viewport_rect.size(),
        );
        app.apply_resize_drag(pointer - egui::vec2(0.0, 10.0));
        assert_eq!(
            app.state_panel.viewport_size.y,
            layout.viewport_rect.height() - 10.0
        );
    }

    #[test]
    fn standalone_panel_keeps_bottom_anchor() {
        let ctx = egui::Context::default();
        let app = QniApp::new(&eframe::CreationContext::_new_kittest(ctx));
        let layout = app.state_panel_layout(small_embed_rect(), 4);
        assert_eq!(
            layout.state_rect.max.y,
            CANVAS_HEIGHT - STATE_CIRCLE_BOTTOM_MARGIN
        );
    }
}
