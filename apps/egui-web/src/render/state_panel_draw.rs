//! State-vector panel drawing — `draw_state_vector` plus the helpers
//! it dispatches to (popover, minimap, resize-handle arc). Reads layout
//! info from `state_panel_layout` and paints with the `egui::Painter`.

use eframe::egui;
use eframe::{egui_wgpu, wgpu};

use crate::app::{QniApp, ResizeCorner};
use crate::colors::Colors;
use crate::constants::{
    STATE_PANEL_CORNER_RADIUS, STATE_RESIZE_HANDLE_PAD, STATE_RESIZE_HANDLE_STROKE,
};
use crate::gpu::{RenderColors, StateVectorCallback};
use super::state_panel_layout::StatePanelLayout;

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
        recompute: bool,
        target_format: Option<wgpu::TextureFormat>,
    ) -> egui::Rect {
        let state_rect = layout.state_rect.translate(offset);
        let viewport_rect = layout.viewport_rect.translate(offset);
        // Where the circle grid actually lands in viewport coords. Centred
        // when the grid fits, panned by `state_grid_offset` otherwise.
        let grid_origin = Self::grid_origin(layout, offset, self.state_grid_offset);
        let state_corner = egui::CornerRadius::same(14);
        let state_shadow = egui::epaint::Shadow {
            offset: [0, 6],
            blur: 16,
            spread: 0,
            color: egui::Color32::from_rgba_unmultiplied(0, 0, 0, 25),
        };
        painter.add(egui::Shape::Rect(
            state_shadow.as_shape(state_rect, state_corner),
        ));
        painter.rect_filled(state_rect, state_corner, colors.surface);

        // G-2 header strip: zinc-100 bar with qubit count on the left and
        // "cols × rows = N states" on the right. Top corners follow the
        // panel's corner radius; the bottom edge is flat where the strip
        // meets the white panel body.
        let handle_rect = egui::Rect::from_min_size(
            state_rect.min,
            egui::vec2(state_rect.width(), handle_height.max(6.0)),
        );
        let handle_corner = egui::CornerRadius {
            nw: 14,
            ne: 14,
            sw: 0,
            se: 0,
        };
        painter.rect_filled(handle_rect, handle_corner, colors.state_handle_bg);

        // Strip text starts past the corner resize-handle area
        // (panel rounded R + breathing). Keeps "16 qubits" / "256 × 256 = …"
        // from touching the curved handle marks at the top corners.
        let strip_padding_x = STATE_PANEL_CORNER_RADIUS + 6.0;
        let strip_font = egui::FontId::monospace(11.0);
        let qubits_label = if layout.qubits == 1 { "qubit" } else { "qubits" };
        let states_label = if layout.state_count == 1 { "state" } else { "states" };
        let qubits_text = format!("{} {}", layout.qubits, qubits_label);
        let rows = layout.state_count / layout.columns.max(1);
        // " ▾" indicates the dimensions text opens the aspect popover.
        let states_text = format!(
            "{} × {} = {} {} ▾",
            layout.columns, rows, layout.state_count, states_label
        );
        // sky-500 strip → white text for legibility.
        painter.text(
            handle_rect.left_center() + egui::vec2(strip_padding_x, 0.0),
            egui::Align2::LEFT_CENTER,
            qubits_text,
            strip_font.clone(),
            colors.surface,
        );
        painter.text(
            handle_rect.right_center() - egui::vec2(strip_padding_x, 0.0),
            egui::Align2::RIGHT_CENTER,
            states_text,
            strip_font,
            colors.surface,
        );

        if let Some(target_format) = target_format {
            let sim_ops = if recompute {
                self.sim_ops.clone()
            } else {
                Vec::new()
            };
            let render_colors = RenderColors::new(colors);
            let callback_rect = screen_rect;
            let cell_pitch = layout.size + layout.gap;
            let cols = layout.columns as u32;
            let rows = (layout.state_count / layout.columns.max(1)) as u32;
            let render_params = crate::gpu::RenderParams {
                viewport_min: [callback_rect.min.x, callback_rect.min.y],
                viewport_size: [callback_rect.width(), callback_rect.height()],
                panel_origin: [grid_origin.x, grid_origin.y],
                panel_size: [cols as f32 * cell_pitch, rows as f32 * cell_pitch],
                cell_pitch,
                radius: layout.radius,
                inner_radius: layout.inner_radius,
                stroke: layout.stroke,
                cols,
                rows,
                qubits: layout.qubits as u32,
                hovered_cell: self.hovered_state_cell.map_or(-1, |c| c as i32),
                surface: render_colors.surface,
                fill: render_colors.fill,
                outline: render_colors.outline,
                outline_zero: render_colors.outline_zero,
                needle: render_colors.needle,
            };
            let callback = StateVectorCallback {
                sim_ops,
                state_count: layout.state_count,
                recompute,
                target_format,
                render_params,
            };
            // Clip the GPU pass to the viewport so the grid is cropped at
            // the panel body's inner edge — circles flush against the rounded
            // corners get sliced cleanly instead of bleeding past the panel.
            let clipped = painter.with_clip_rect(viewport_rect);
            let paint_callback = egui_wgpu::Callback::new_paint_callback(callback_rect, callback);
            clipped.add(egui::Shape::Callback(paint_callback));
        }

        Self::draw_state_minimap(painter, layout, viewport_rect, grid_origin);

        // Aspect popover (D 案) — only draw when open. Positioned below
        // the dimensions text; floats above the panel and any minimap.
        if self.aspect_popover_open {
            let dims_rect = Self::dims_hit_rect(painter.ctx(), layout, offset);
            let (pop_rect, row_rects) = Self::aspect_popover_layout(dims_rect, layout.qubits);
            Self::draw_aspect_popover(
                painter,
                colors,
                pop_rect,
                &row_rects,
                layout.qubits,
                self.aspect_index.min(layout.qubits),
            );
        }

        // Resize handles — 4 corner arcs concentric with the panel's
        // rounded corners (G 案 / 内側配置). Drawn after the GPU pass so
        // they sit on top of the circle grid at all zoom levels. Color
        // follows the local background: sky-tone for the top handles (on
        // the sky-500 strip), neutral gray for the bottom handles (on the
        // white panel).
        for corner in [
            ResizeCorner::TopLeft,
            ResizeCorner::TopRight,
            ResizeCorner::BottomLeft,
            ResizeCorner::BottomRight,
        ] {
            let dragging = self.active_resize_corner() == Some(corner);
            let color = match (corner.is_top(), dragging) {
                (true, false) => colors.state_resize_handle_top_idle,
                (true, true) => colors.state_resize_handle_top_drag,
                (false, false) => colors.state_resize_handle_bottom_idle,
                (false, true) => colors.state_resize_handle_bottom_drag,
            };
            Self::draw_resize_handle_arc(painter, corner, state_rect, color);
        }

        handle_rect
    }

    /// Draw the aspect popover (background + rows). Each row shows an
    /// aspect-correct thumbnail rect, the cols × rows label, and a "(now)"
    /// tag for the current selection. Fixed-height popover with up to
    /// qubits+1 rows; for 16 qubits that's 17 rows × 22 px ≈ 374 px,
    /// which fits inside `MAX_HEIGHT = 420`.
    pub(super) fn draw_aspect_popover(
        painter: &egui::Painter,
        colors: &Colors,
        rect: egui::Rect,
        rows: &[egui::Rect],
        qubits: usize,
        current_aspect: usize,
    ) {
        // Drop shadow behind the popover so it lifts above the panel.
        let corner = egui::CornerRadius::same(10);
        let shadow = egui::epaint::Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: egui::Color32::from_rgba_unmultiplied(0, 0, 0, 46),
        };
        painter.add(egui::Shape::Rect(shadow.as_shape(rect, corner)));
        painter.rect_filled(rect, corner, colors.surface);

        let label_font = egui::FontId::monospace(12.0);
        const THUMB_SLOT_W: f32 = 50.0;
        const THUMB_SLOT_H: f32 = 16.0;
        for (i, row_rect) in rows.iter().enumerate() {
            let is_current = i == current_aspect;
            let cols = 1usize << i;
            let layout_rows = 1usize << (qubits - i);
            // Row background (current = sky-500, else hover-ready surface).
            if is_current {
                painter.rect_filled(*row_rect, egui::CornerRadius::same(6), colors.state_fill);
            }
            // Thumbnail (aspect-correct rect) inside a fixed 50×16 slot.
            let slot_min = egui::pos2(
                row_rect.min.x + 8.0,
                row_rect.center().y - THUMB_SLOT_H / 2.0,
            );
            let slot_rect =
                egui::Rect::from_min_size(slot_min, egui::vec2(THUMB_SLOT_W, THUMB_SLOT_H));
            let aspect_scale =
                (THUMB_SLOT_W / cols as f32).min(THUMB_SLOT_H / layout_rows as f32);
            let thumb_w = (cols as f32 * aspect_scale).max(1.0);
            let thumb_h = (layout_rows as f32 * aspect_scale).max(1.0);
            let thumb_min = egui::pos2(
                slot_rect.center().x - thumb_w / 2.0,
                slot_rect.center().y - thumb_h / 2.0,
            );
            let thumb_color = if is_current {
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 220)
            } else {
                egui::Color32::from_rgba_unmultiplied(111, 110, 105, 180) // Flexoki tx-2 #6F6E69 70%
            };
            painter.rect_filled(
                egui::Rect::from_min_size(thumb_min, egui::vec2(thumb_w, thumb_h)),
                egui::CornerRadius::ZERO,
                thumb_color,
            );
            // Label
            let label = format!("{} × {}", cols, layout_rows);
            let label_color = if is_current {
                egui::Color32::WHITE
            } else {
                colors.text
            };
            painter.text(
                egui::pos2(row_rect.min.x + 8.0 + THUMB_SLOT_W + 12.0, row_rect.center().y),
                egui::Align2::LEFT_CENTER,
                label,
                label_font.clone(),
                label_color,
            );
            // "(now)" tag for the current row — visible without depending
            // on the ✓ glyph (Hack font ships a generic box for it).
            if is_current {
                painter.text(
                    egui::pos2(row_rect.max.x - 10.0, row_rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    "(now)",
                    egui::FontId::monospace(10.0),
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 220),
                );
            }
        }
    }

    /// Draw a single resize-handle arc. The arc is concentric with the
    /// panel's rounded corner: same centre as the panel's corner-radius
    /// circle, radius = `STATE_PANEL_CORNER_RADIUS − STATE_RESIZE_HANDLE_PAD`.
    /// This means the handle literally traces the panel's rounded inner
    /// edge offset inward by `PAD`, so the handle's curvature matches the
    /// panel's R exactly.
    fn draw_resize_handle_arc(
        painter: &egui::Painter,
        corner: ResizeCorner,
        state_rect: egui::Rect,
        color: egui::Color32,
    ) {
        use std::f32::consts::PI;
        let r = STATE_PANEL_CORNER_RADIUS;
        let inner_r = (r - STATE_RESIZE_HANDLE_PAD).max(0.0);
        if inner_r <= 0.0 {
            return;
        }
        // `center` is the centre of the panel's rounded-corner circle for
        // this corner (located `r` px inside the corner along both axes).
        // `start_angle` is the angle on the circle at which the arc begins;
        // we always sweep +π/2 (90°) counterclockwise (in math y-up terms;
        // visually that's "along the corner curve").
        let (center, start_angle) = match corner {
            ResizeCorner::TopLeft => (state_rect.min + egui::vec2(r, r), PI),
            ResizeCorner::TopRight => {
                (egui::pos2(state_rect.max.x - r, state_rect.min.y + r), -PI / 2.0)
            }
            ResizeCorner::BottomRight => (state_rect.max - egui::vec2(r, r), 0.0),
            ResizeCorner::BottomLeft => {
                (egui::pos2(state_rect.min.x + r, state_rect.max.y - r), PI / 2.0)
            }
        };
        const ARC_SEGMENTS: usize = 16;
        let mut points: Vec<egui::Pos2> = Vec::with_capacity(ARC_SEGMENTS + 1);
        for i in 0..=ARC_SEGMENTS {
            let t = i as f32 / ARC_SEGMENTS as f32;
            let a = start_angle + t * (PI / 2.0);
            points.push(center + egui::vec2(inner_r * a.cos(), inner_r * a.sin()));
        }
        painter.add(egui::Shape::line(
            points,
            egui::Stroke::new(STATE_RESIZE_HANDLE_STROKE, color),
        ));
    }

    /// Bottom-right minimap that shows where the viewport is sitting on the
    /// (potentially much larger) grid. The outer rectangle matches the grid
    /// aspect (so a 32×32 grid produces a square minimap, a 32×8 grid a
    /// wide one), and the lighter inset rectangle inside marks the visible
    /// region. Only painted when the grid actually exceeds the viewport on
    /// at least one axis — otherwise the whole grid is on screen and the
    /// minimap is just chrome.
    fn draw_state_minimap(
        painter: &egui::Painter,
        layout: &StatePanelLayout,
        viewport_rect: egui::Rect,
        grid_origin: egui::Pos2,
    ) {
        let grid = layout.grid_size;
        if grid.x <= viewport_rect.width() && grid.y <= viewport_rect.height() {
            return;
        }
        // Grid aspect drives the minimap's outer dimensions, capped at a
        // bounding box. No letterboxing inside — the inset rect IS the grid.
        const MAX_W: f32 = 80.0;
        const MAX_H: f32 = 50.0;
        const INSET: f32 = 3.0;
        let aspect = grid.x / grid.y;
        let (inner_w, inner_h) = if aspect >= MAX_W / MAX_H {
            (MAX_W, MAX_W / aspect)
        } else {
            (MAX_H * aspect, MAX_H)
        };
        let mm_size = egui::vec2(inner_w + INSET * 2.0, inner_h + INSET * 2.0);
        // Small inset past the BR resize-handle arc (~14 px from the
        // corner) — just enough to break the visual "stuck together"
        // but not so far it floats in space.
        let pad = 12.0;
        let mm_rect = egui::Rect::from_min_max(
            viewport_rect.max - mm_size - egui::vec2(pad, pad),
            viewport_rect.max - egui::vec2(pad, pad),
        );
        let bg = egui::Color32::from_rgba_unmultiplied(0, 0, 0, 140);
        painter.rect_filled(mm_rect, egui::CornerRadius::same(4), bg);

        let mm_grid_min = mm_rect.min + egui::vec2(INSET, INSET);
        let mm_grid_size = egui::vec2(inner_w, inner_h);
        let scale = inner_w / grid.x;
        // Visible region inside the grid, in grid-space pixels.
        let visible_offset = viewport_rect.min - grid_origin;
        let mm_vp_min = mm_grid_min + visible_offset * scale;
        let mm_vp_size = egui::vec2(viewport_rect.width(), viewport_rect.height()) * scale;
        let mm_vp_rect = egui::Rect::from_min_size(mm_vp_min, mm_vp_size)
            .intersect(egui::Rect::from_min_size(mm_grid_min, mm_grid_size));
        let vp_fill = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 70);
        let vp_stroke = egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 220),
        );
        painter.rect_filled(mm_vp_rect, egui::CornerRadius::ZERO, vp_fill);
        painter.rect_stroke(
            mm_vp_rect,
            egui::CornerRadius::ZERO,
            vp_stroke,
            egui::StrokeKind::Inside,
        );
    }
}
