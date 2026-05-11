//! State-vector panel geometry — `StatePanelLayout` struct, layout
//! computation, hit-rect calculations for the strip / dims text /
//! resize handles / aspect popover, plus offset clamps. Pure geometry,
//! no `Painter` calls.

use eframe::egui;

use crate::app::{QniApp, ResizeCorner};
use crate::constants::{
    state_circle_layout, STATE_CIRCLE_BOTTOM_MARGIN, STATE_HANDLE_HEIGHT,
    STATE_PANEL_CORNER_RADIUS, STATE_RESIZE_HIT_PAD,
};
use crate::shared::amplitude_qubits;

pub(crate) struct StatePanelLayout {
    pub(super) state_count: usize,
    pub(super) qubits: usize,
    pub(super) columns: usize,
    pub(super) size: f32,
    pub(super) gap: f32,
    pub(super) radius: f32,
    pub(super) stroke: f32,
    pub(super) inner_radius: f32,
    /// Total pixel size of the circle grid (cols × cell_pitch, rows × cell_pitch).
    pub(super) grid_size: egui::Vec2,
    /// Inner area below the header strip where circles render. Fixed size;
    /// when the grid is smaller it gets centred, when larger it pans inside.
    pub(crate) viewport_rect: egui::Rect,
    pub(crate) state_rect: egui::Rect,
    pub(crate) handle_height: f32,
}

impl QniApp {
    pub(crate) fn state_panel_layout(
        &self,
        rect: egui::Rect,
        state_count: usize,
    ) -> StatePanelLayout {
        let state_count = state_count.max(1);
        let qubits = amplitude_qubits(state_count);

        // Cell size + line width follow qni's per-qubit-count table; the
        // (cols, rows) split is parameterised by `self.aspect_index` so
        // the user can change the layout aspect at runtime. qni's
        // reference uses gap == stroke (cells touch); we add 1 px so
        // adjacent stroke rings don't share a pixel boundary at dist ==
        // outer. Without this slack the GPU-side single-cell render
        // gives 50 % alpha at the boundary (symmetric smoothstep midpoint
        // is exactly 0.5), visibly fading the outline. The 1-px seam is
        // barely perceptible at typical zoom and lets us keep V-sync at
        // 11+ qubits without paying for 2x2 cell sampling in the
        // fragment shader.
        let qni = state_circle_layout(qubits, self.aspect_index);
        let columns = qni.cols;
        let rows = qni.rows;
        // Zoom scales every length-y thing in the grid uniformly so cells
        // grow / shrink together. Stroke has a 0.5 px floor so very-zoomed-
        // out cells still get a visible outline rather than collapsing into
        // pure fill.
        let zoom = self.state_grid_zoom;
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
        // labels never overlap. Hack monospace at 11 px is ≈7 px / glyph;
        // budget a bit extra for the multiplication sign.
        const STRIP_CHAR_WIDTH: f32 = 7.0;
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
        let panel_width = self.state_viewport_size.x.max(strip_min_width);
        let panel_height = self.state_viewport_size.y + handle_height;
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
    /// layout and the user's pan offset. If the grid fits inside the
    /// viewport on an axis it gets centred (and the pan offset is ignored
    /// for that axis); otherwise the offset is clamped so the grid can pan
    /// only as far as its edges meet the viewport edges.
    pub(crate) fn grid_origin(
        layout: &StatePanelLayout,
        viewport_offset: egui::Vec2,
        pan: egui::Vec2,
    ) -> egui::Pos2 {
        let viewport = layout.viewport_rect.translate(viewport_offset);
        let grid = layout.grid_size;
        let origin_x = if grid.x <= viewport.width() {
            viewport.min.x + (viewport.width() - grid.x) / 2.0
        } else {
            // Grid wider than viewport — clamp pan so the grid edges can't
            // separate from the viewport edges (no empty bands either side).
            let min = viewport.max.x - grid.x;
            let max = viewport.min.x;
            (viewport.min.x + pan.x).clamp(min, max)
        };
        let origin_y = if grid.y <= viewport.height() {
            viewport.min.y + (viewport.height() - grid.y) / 2.0
        } else {
            let min = viewport.max.y - grid.y;
            let max = viewport.min.y;
            (viewport.min.y + pan.y).clamp(min, max)
        };
        egui::pos2(origin_x, origin_y)
    }

    /// Hit rect for the strip's dimensions text ("C × R = N states ▾"),
    /// which is wheel-scrollable for aspect ±1 and click-able to open the
    /// aspect popover. Computed by measuring the text with the actual font
    /// so the rect exactly tracks the rendered glyphs; expanded by a few
    /// px on all sides for forgiving clicks.
    pub(crate) fn dims_text(layout: &StatePanelLayout) -> String {
        let states_label = if layout.state_count == 1 {
            "state"
        } else {
            "states"
        };
        let rows = layout.state_count / layout.columns.max(1);
        format!(
            "{} × {} = {} {} ▾",
            layout.columns, rows, layout.state_count, states_label
        )
    }

    pub(crate) fn dims_hit_rect(
        ctx: &egui::Context,
        layout: &StatePanelLayout,
        state_panel_offset: egui::Vec2,
    ) -> egui::Rect {
        let state_rect = layout.state_rect.translate(state_panel_offset);
        let handle_rect = egui::Rect::from_min_size(
            state_rect.min,
            egui::vec2(state_rect.width(), layout.handle_height.max(6.0)),
        );
        let strip_padding_x = STATE_PANEL_CORNER_RADIUS + 6.0;
        let font = egui::FontId::monospace(11.0);
        let text = Self::dims_text(layout);
        let size = ctx.fonts_mut(|f| {
            f.layout_no_wrap(text, font, egui::Color32::WHITE).size()
        });
        let right_center = handle_rect.right_center() - egui::vec2(strip_padding_x, 0.0);
        let visible = egui::Rect::from_min_max(
            egui::pos2(right_center.x - size.x, right_center.y - size.y / 2.0),
            egui::pos2(right_center.x, right_center.y + size.y / 2.0),
        );
        visible.expand2(egui::vec2(6.0, 4.0))
    }

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

    /// Hit rect for grabbing one corner. The visible handle is an arc of
    /// the panel's rounded inner edge, but for clicks we expose the full
    /// `R × R` square at the corner (the panel's rounded-corner bounding
    /// box) inflated by `STATE_RESIZE_HIT_PAD` so the corner is forgiving
    /// to grab even when the cursor isn't right on the curve.
    pub(crate) fn resize_handle_hit_rect(
        layout: &StatePanelLayout,
        offset: egui::Vec2,
        corner: ResizeCorner,
    ) -> egui::Rect {
        let state_rect = layout.state_rect.translate(offset);
        let r = STATE_PANEL_CORNER_RADIUS;
        let base = match corner {
            ResizeCorner::TopLeft => {
                egui::Rect::from_min_size(state_rect.min, egui::vec2(r, r))
            }
            ResizeCorner::TopRight => egui::Rect::from_min_size(
                egui::pos2(state_rect.max.x - r, state_rect.min.y),
                egui::vec2(r, r),
            ),
            ResizeCorner::BottomLeft => egui::Rect::from_min_size(
                egui::pos2(state_rect.min.x, state_rect.max.y - r),
                egui::vec2(r, r),
            ),
            ResizeCorner::BottomRight => egui::Rect::from_min_size(
                egui::pos2(state_rect.max.x - r, state_rect.max.y - r),
                egui::vec2(r, r),
            ),
        };
        base.expand(STATE_RESIZE_HIT_PAD)
    }

    pub(crate) fn clamp_state_panel_offset(
        &mut self,
        layout: &StatePanelLayout,
        rect: egui::Rect,
    ) {
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

        self.state_panel_offset.x = if max_offset_x < min_offset_x {
            min_offset_x
        } else {
            self.state_panel_offset.x.clamp(min_offset_x, max_offset_x)
        };
        self.state_panel_offset.y = if max_offset_y < min_offset_y {
            min_offset_y
        } else {
            self.state_panel_offset.y.clamp(min_offset_y, max_offset_y)
        };
    }

    /// Keep `state_grid_offset` inside the range that lets `grid_origin`
    /// produce a non-flickering value: 0 when the grid fits on an axis,
    /// `[viewport - grid, 0]` when it overflows. Called every frame after
    /// the layout is computed so qubit-count changes don't leave a stale
    /// (possibly huge) pan offset around.
    pub(crate) fn clamp_state_grid_offset(&mut self, layout: &StatePanelLayout) {
        let viewport = layout.viewport_rect.translate(self.state_panel_offset);
        let grid = layout.grid_size;
        self.state_grid_offset.x = if grid.x <= viewport.width() {
            0.0
        } else {
            self.state_grid_offset
                .x
                .clamp(viewport.width() - grid.x, 0.0)
        };
        self.state_grid_offset.y = if grid.y <= viewport.height() {
            0.0
        } else {
            self.state_grid_offset
                .y
                .clamp(viewport.height() - grid.y, 0.0)
        };
    }
}
