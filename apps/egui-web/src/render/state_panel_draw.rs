//! State-vector panel drawing — `draw_state_vector` plus the helpers
//! it dispatches to (popover, minimap, resize-handle arc). Reads layout
//! info from `state_panel_layout` and paints with the `egui::Painter`.

use eframe::egui;
use eframe::{egui_wgpu, wgpu};

use super::state_panel_layout::StatePanelLayout;
use super::{state_panel_chrome, state_panel_popup};
use crate::app::{QniApp, ResizeCorner};
use crate::colors::Colors;
use crate::constants::STATE_PANEL_CORNER_RADIUS;
use crate::gpu::{RenderColors, StateVectorCallback};

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
        // text-sm (14px) — Tailwind default. Matches the popup header so
        // both blue-on-paper "card chrome" text sits at the same step on
        // the type scale.
        let strip_font = egui::FontId::monospace(14.0);
        let qubits_label = if layout.qubits == 1 {
            "qubit"
        } else {
            "qubits"
        };
        let states_label = if layout.state_count == 1 {
            "state"
        } else {
            "states"
        };
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

        state_panel_chrome::draw_state_minimap(painter, layout, viewport_rect, grid_origin);

        // Aspect popover (D 案) — only draw when open. Positioned below
        // the dimensions text; floats above the panel and any minimap.
        if self.aspect_popover_open {
            let dims_rect = Self::dims_hit_rect(painter.ctx(), layout, offset);
            let (pop_rect, row_rects) = Self::aspect_popover_layout(dims_rect, layout.qubits);
            state_panel_popup::draw_aspect_popover(
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
            state_panel_chrome::draw_resize_handle_arc(painter, corner, state_rect, color);
        }

        // Cell hover popup (B — Paper + ui-2 1px border + shadow, with
        // qni-style amplitude / probability / phase icons). Drawn last
        // so it lifts above the resize handles + minimap + GPU circle
        // pass.
        if let Some(cell) = self.hovered_state_cell {
            let grid_origin = Self::grid_origin(layout, offset, self.state_grid_offset);
            state_panel_popup::draw_state_cell_popup(
                painter,
                colors,
                layout,
                grid_origin,
                viewport_rect,
                screen_rect,
                cell,
            );
        }

        handle_rect
    }
}
