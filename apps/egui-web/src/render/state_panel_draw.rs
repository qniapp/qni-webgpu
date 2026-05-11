//! State-vector panel drawing — `draw_state_vector` plus the helpers
//! it dispatches to (popover, minimap, resize-handle arc). Reads layout
//! info from `state_panel_layout` and paints with the `egui::Painter`.

use eframe::egui;
use eframe::{egui_wgpu, wgpu};

use super::state_panel_layout::StatePanelLayout;
use crate::app::{QniApp, ResizeCorner};
use crate::colors::Colors;
use crate::constants::{
    STATE_PANEL_CORNER_RADIUS, STATE_RESIZE_HANDLE_PAD, STATE_RESIZE_HANDLE_STROKE,
};
use crate::gpu::{
    PopupValueCallback, RenderColors, StateVectorCallback, POPUP_GLYPH_CELL_H, POPUP_GLYPH_CELL_W,
};

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

        // Cell hover popup (B — Paper + ui-2 1px border + shadow, with
        // qni-style amplitude / probability / phase icons). Drawn last
        // so it lifts above the resize handles + minimap + GPU circle
        // pass.
        if let Some(cell) = self.hovered_state_cell {
            let grid_origin = Self::grid_origin(layout, offset, self.state_grid_offset);
            Self::draw_state_cell_popup(
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

    /// Hover popup showing the |ket⟩ + amplitude / probability / phase for
    /// the cell under the pointer. Paper background + ui-2 1px border +
    /// soft shadow (B variant from `docs/state-cell-popup-mockups.html`),
    /// with qni-style icons rendered as egui primitives.
    ///
    /// Numeric amplitude / probability / phase values are placeholders
    /// for now — the actual values live on the GPU and reading them
    /// back per-hover requires either a one-shot `copy_buffer_to_buffer`
    /// followed by `map_async` (mild violation of the "no readback in
    /// production" rule but on-demand, not per-frame) or a shader-side
    /// text renderer. Chrome first; wire up values in a follow-up.
    fn draw_state_cell_popup(
        painter: &egui::Painter,
        colors: &Colors,
        layout: &StatePanelLayout,
        grid_origin: egui::Pos2,
        viewport_rect: egui::Rect,
        screen_rect: egui::Rect,
        display_index: u32,
    ) {
        let qubits = layout.qubits.max(1) as u32;
        // Label uses the cell's *display position* (= `display_index`)
        // rather than the bit-reversed state-vector index. Matches
        // qni's `circle-notation-element.ts:907` where
        // `ket = col + row * colCount` — the same `(row, col)` the user
        // is visually hovering. The fragment shader still reads
        // `state[reverse_bits(display_index)]` for the amplitude /
        // probability / phase columns, so the displayed numbers stay
        // consistent with the cell the user is pointing at.
        let ket_binary = format!("{:0width$b}", display_index, width = qubits as usize);
        let header = format!("|{}⟩ decimal {}", ket_binary, display_index);

        let pitch = layout.cell_pitch();
        let cols = layout.columns().max(1);
        let col = (display_index as usize) % cols;
        let row = (display_index as usize) / cols;
        let cell_center = egui::pos2(
            grid_origin.x + col as f32 * pitch + pitch * 0.5,
            grid_origin.y + row as f32 * pitch + pitch * 0.5,
        );

        // Popup geometry, on the Tailwind 4-px spacing scale. Heights are
        // derived from each text size's default Tailwind line-height so
        // the gap above the header and below the last row match exactly.
        //   POPUP_W       — header + body width (no Tailwind preset for
        //                   "tooltip width"; sized to fit the widest row,
        //                   17 chars × 9 px glyph cell + chrome).
        //   POPUP_PAD_X/Y — spacing-4 (16) / spacing-3 (12).
        //   HEADER_TEXT_H — text-sm line-height (20px).
        //   HEADER_GAP    — spacing-2 (8px).
        //   ROW_H         — spacing-5 (20px); also text-sm line-height.
        //   BODY_TEXT_H   — text-xs line-height (16px).
        const POPUP_W: f32 = 296.0;
        const POPUP_PAD_X: f32 = 16.0;
        const POPUP_PAD_Y: f32 = 12.0;
        const HEADER_TEXT_H: f32 = 20.0;
        const HEADER_GAP: f32 = 8.0;
        const ROW_H: f32 = 20.0;
        const BODY_TEXT_H: f32 = 16.0;
        const ROWS: usize = 3;
        let popup_h = POPUP_PAD_Y * 2.0
            + HEADER_TEXT_H
            + HEADER_GAP
            + ROW_H * (ROWS as f32 - 1.0)
            + BODY_TEXT_H;
        const TAIL_H: f32 = 8.0;
        const TAIL_HALF_W: f32 = 8.0;
        const GAP_TO_CELL: f32 = 4.0;

        // Prefer above the cell — the state panel anchors to the bottom
        // of the screen, so "above" usually lands in the empty page
        // area. Only flip below if going above would push the popup
        // past the screen top.
        const SCREEN_TOP_MARGIN: f32 = 8.0;
        let above_top = cell_center.y - layout.radius - GAP_TO_CELL - TAIL_H - popup_h;
        let prefer_above = above_top >= SCREEN_TOP_MARGIN;
        let (popup_rect, tail_apex_y, tail_base_y) = if prefer_above {
            let r = egui::Rect::from_min_size(
                egui::pos2(cell_center.x - POPUP_W * 0.5, above_top),
                egui::vec2(POPUP_W, popup_h),
            );
            (r, r.max.y + TAIL_H, r.max.y)
        } else {
            let top = cell_center.y + layout.radius + GAP_TO_CELL + TAIL_H;
            let r = egui::Rect::from_min_size(
                egui::pos2(cell_center.x - POPUP_W * 0.5, top),
                egui::vec2(POPUP_W, popup_h),
            );
            (r, r.min.y - TAIL_H, r.min.y)
        };
        // Clamp horizontally so the popup never falls off the viewport.
        let clamped = {
            let mut r = popup_rect;
            if r.min.x < viewport_rect.min.x + 4.0 {
                let dx = viewport_rect.min.x + 4.0 - r.min.x;
                r = r.translate(egui::vec2(dx, 0.0));
            } else if r.max.x > viewport_rect.max.x - 4.0 {
                let dx = viewport_rect.max.x - 4.0 - r.max.x;
                r = r.translate(egui::vec2(dx, 0.0));
            }
            r
        };

        // Drop shadow → paper fill → ui-2 hairline border (B variant).
        let corner = egui::CornerRadius::same(10);
        let shadow = egui::epaint::Shadow {
            offset: [0, 10],
            blur: 28,
            spread: 0,
            color: egui::Color32::from_rgba_unmultiplied(0, 0, 0, 36),
        };
        painter.add(egui::Shape::Rect(shadow.as_shape(clamped, corner)));
        painter.rect_filled(clamped, corner, colors.surface);
        painter.rect_stroke(
            clamped,
            corner,
            egui::Stroke::new(1.0, colors.state_outline_zero),
            egui::StrokeKind::Inside,
        );

        // Tail (small triangle pointing at the cell). Filled paper +
        // matching ui-2 stroke on the two slanted sides so it reads as
        // part of the bordered card. Uses the un-clamped horizontal
        // anchor (cell_center.x) so the apex always lands on the cell.
        let apex = egui::pos2(cell_center.x, tail_apex_y);
        let base_l = egui::pos2(cell_center.x - TAIL_HALF_W, tail_base_y);
        let base_r = egui::pos2(cell_center.x + TAIL_HALF_W, tail_base_y);
        painter.add(egui::Shape::convex_polygon(
            vec![apex, base_l, base_r],
            colors.surface,
            egui::Stroke::NONE,
        ));
        let border_stroke = egui::Stroke::new(1.0, colors.state_outline_zero);
        painter.line_segment([base_l, apex], border_stroke);
        painter.line_segment([apex, base_r], border_stroke);
        // Repaint the popup-body edge between the tail bases so the
        // border doesn't show through under the tail.
        painter.line_segment(
            [
                egui::pos2(cell_center.x - TAIL_HALF_W + 0.5, tail_base_y),
                egui::pos2(cell_center.x + TAIL_HALF_W - 0.5, tail_base_y),
            ],
            egui::Stroke::new(1.0, colors.surface),
        );

        // Text content — header + 3 rows. Body text uses tx (near-black)
        // for full readability; labels share the same colour so the row
        // doesn't fade. Icons use tx-2 (a half-step lighter) to read as
        // chrome rather than data.
        // text-sm (14px) for the |ket⟩ header, text-xs (12px) for the
        // amplitude/probability/phase rows. Both are Tailwind defaults.
        let header_font = egui::FontId::monospace(14.0);
        let body_font = egui::FontId::monospace(12.0);
        // Label / value contrast mirrors the mock: labels in tx-2
        // (`state_outline`, light gray) and values in tx
        // (`state_needle`, near-black) so the eye lands on the numbers.
        // egui's default mono has no bold weight, so weight contrast is
        // expressed entirely through colour.
        let label_color = colors.state_outline;
        let value_color = colors.state_needle;
        // Each icon is two-tone: chrome (tx-3 light gray) for the
        // supporting frame, accent (blue-400) for a single "one-point"
        // element — the arrow shaft + head for amplitude, the inner
        // disk for probability, the arc for phase. The chrome / accent
        // split makes the data part of each glyph pop without making
        // the whole icon a bright primary colour.
        let icon_accent = colors.popup_icon;
        let icon_chrome = colors.popup_icon_chrome;
        let header_y = clamped.min.y + POPUP_PAD_Y;
        painter.text(
            egui::pos2(clamped.min.x + POPUP_PAD_X, header_y),
            egui::Align2::LEFT_TOP,
            header,
            header_font,
            colors.state_needle,
        );
        let labels = ["Amplitude:", "Probability:", "Phase:"];
        let row_0_y = header_y + HEADER_TEXT_H + HEADER_GAP;
        const ICON_SIZE: f32 = 16.0;
        const ICON_TEXT_GAP: f32 = 8.0;
        const LABEL_X_OFFSET: f32 = ICON_SIZE + ICON_TEXT_GAP;
        const VALUE_X_OFFSET: f32 = LABEL_X_OFFSET + 96.0;
        for (i, label) in labels.iter().enumerate() {
            let y = row_0_y + (i as f32) * ROW_H;
            let icon_rect = egui::Rect::from_min_size(
                egui::pos2(clamped.min.x + POPUP_PAD_X, y + (ROW_H - ICON_SIZE) * 0.5),
                egui::vec2(ICON_SIZE, ICON_SIZE),
            );
            match i {
                0 => Self::draw_amplitude_icon(painter, icon_rect, icon_chrome, icon_accent),
                1 => Self::draw_probability_icon(painter, icon_rect, icon_chrome, icon_accent),
                _ => Self::draw_phase_icon(painter, icon_rect, icon_chrome, icon_accent),
            }
            painter.text(
                egui::pos2(clamped.min.x + POPUP_PAD_X + LABEL_X_OFFSET, y),
                egui::Align2::LEFT_TOP,
                *label,
                body_font.clone(),
                label_color,
            );
        }
        // Numeric values come from the GPU state buffer via a dedicated
        // render pass — see `PopupValueCallback`. The CPU only computes
        // the anchor / row pitch / colour and hands them off; no readback.
        let value_anchor = egui::pos2(
            clamped.min.x + POPUP_PAD_X + VALUE_X_OFFSET,
            // Egui draws monospace text with the cap-height a touch below
            // the cell top; nudge the atlas anchor up so the digits sit
            // on the same baseline as the labels to their left.
            row_0_y - 2.0,
        );
        let popup_value_callback = PopupValueCallback {
            viewport_min: [screen_rect.min.x, screen_rect.min.y],
            viewport_size: [screen_rect.width(), screen_rect.height()],
            value_anchor: [value_anchor.x, value_anchor.y],
            row_pitch: ROW_H,
            char_size: [POPUP_GLYPH_CELL_W as f32, POPUP_GLYPH_CELL_H as f32],
            text_color: [
                value_color.r() as f32 / 255.0,
                value_color.g() as f32 / 255.0,
                value_color.b() as f32 / 255.0,
                1.0,
            ],
            hovered_display_index: display_index,
            qubits: layout.qubits as u32,
        };
        // Clip the GPU pass to the popup body so the value text never
        // bleeds past the border / tail.
        let value_rect = egui::Rect::from_min_size(
            value_anchor,
            egui::vec2(
                POPUP_GLYPH_CELL_W as f32 * 17.0, // widest row = amplitude (17 chars)
                ROW_H * 3.0,
            ),
        );
        let paint_callback =
            egui_wgpu::Callback::new_paint_callback(screen_rect, popup_value_callback);
        let clipped = painter.with_clip_rect(value_rect);
        clipped.add(egui::Shape::Callback(paint_callback));
    }

    /// Amplitude icon — Re/Im axes with the origin pushed into the
    /// lower-left corner so the first quadrant fills ~75% of the
    /// frame. That leaves room for a large diagonal arrow representing
    /// the complex amplitude. Origin at (3, 13); axes stay thin so the
    /// arrow reads as foreground. Arrow shaft 2.2 px, filled triangular
    /// head.
    fn draw_amplitude_icon(
        painter: &egui::Painter,
        rect: egui::Rect,
        chrome: egui::Color32,
        accent: egui::Color32,
    ) {
        let s = rect.width() / 16.0;
        let axis_stroke = egui::Stroke::new(1.2 * s, chrome);
        let shaft = egui::Stroke::new(2.2 * s, accent);
        let origin = egui::pos2(rect.min.x + 3.0 * s, rect.min.y + 13.0 * s);
        // Re axis (horizontal at y = 13). A tiny stub on the left
        // (x = 1..3) hints at the negative region; the bulk (x = 3..15)
        // is the +Re half-line.
        painter.line_segment(
            [
                egui::pos2(rect.min.x + 1.0 * s, origin.y),
                egui::pos2(rect.min.x + 15.0 * s, origin.y),
            ],
            axis_stroke,
        );
        // Im axis (vertical at x = 3). Stub below (y = 13..15) and the
        // bulk (y = 1..13) is +Im.
        painter.line_segment(
            [
                egui::pos2(origin.x, rect.min.y + 1.0 * s),
                egui::pos2(origin.x, rect.min.y + 15.0 * s),
            ],
            axis_stroke,
        );
        // Origin dot — chrome, supporting role.
        painter.circle_filled(origin, 1.4 * s, chrome);
        // Arrow shaft + head — blue accent. Tip near (14, 2).
        let tip = egui::pos2(rect.min.x + 14.0 * s, rect.min.y + 2.0 * s);
        let dir = (tip - origin).normalized();
        // Stop the shaft well short of the tip so the head reads as a
        // distinct triangle, not a thick stub on the shaft.
        let head_len = 5.2 * s;
        let head_half = 3.4 * s;
        let shaft_end = tip - dir * (head_len - 0.4 * s);
        painter.line_segment([origin, shaft_end], shaft);
        // Arrowhead — filled triangle in accent. perp is 90° from dir.
        let perp = egui::vec2(dir.y, -dir.x);
        let base_centre = tip - dir * head_len;
        let base_l = base_centre + perp * head_half;
        let base_r = base_centre - perp * head_half;
        painter.add(egui::Shape::convex_polygon(
            vec![tip, base_l, base_r],
            accent,
            egui::Stroke::NONE,
        ));
    }

    /// Probability icon — chrome outer ring + accent (blue) inner disk.
    /// The blue inner disk reads as "the value" (probability mass);
    /// the gray ring frames it without competing.
    fn draw_probability_icon(
        painter: &egui::Painter,
        rect: egui::Rect,
        chrome: egui::Color32,
        accent: egui::Color32,
    ) {
        let s = rect.width() / 16.0;
        let centre = egui::pos2(rect.min.x + 8.0 * s, rect.min.y + 8.0 * s);
        painter.circle_stroke(centre, 6.4 * s, egui::Stroke::new(2.0 * s, chrome));
        painter.circle_filled(centre, 4.0 * s, accent);
    }

    /// Phase icon — chrome base line + hypotenuse with an accent (blue)
    /// arc near the origin marking the swept angle. The arc is the
    /// "the value" (phase angle); the two straight lines just frame the
    /// triangle.
    fn draw_phase_icon(
        painter: &egui::Painter,
        rect: egui::Rect,
        chrome: egui::Color32,
        accent: egui::Color32,
    ) {
        let s = rect.width() / 16.0;
        let chrome_stroke = egui::Stroke::new(1.6 * s, chrome);
        let accent_stroke = egui::Stroke::new(2.0 * s, accent);
        let base_l = egui::pos2(rect.min.x + 2.5 * s, rect.min.y + 12.6 * s);
        let base_r = egui::pos2(rect.min.x + 14.2 * s, rect.min.y + 12.6 * s);
        let hyp_top = egui::pos2(rect.min.x + 10.23 * s, rect.min.y + 3.09 * s);
        painter.line_segment([base_l, base_r], chrome_stroke);
        painter.line_segment([base_l, hyp_top], chrome_stroke);
        // Arc near the origin — approximated as a short polyline, drawn
        // in the blue accent so the "phase angle" reads as the subject.
        let arc_pts = [
            egui::pos2(rect.min.x + 6.02 * s, rect.min.y + 8.95 * s),
            egui::pos2(rect.min.x + 7.10 * s, rect.min.y + 10.60 * s),
            egui::pos2(rect.min.x + 7.90 * s, rect.min.y + 12.95 * s),
        ];
        painter.line_segment([arc_pts[0], arc_pts[1]], accent_stroke);
        painter.line_segment([arc_pts[1], arc_pts[2]], accent_stroke);
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
            let aspect_scale = (THUMB_SLOT_W / cols as f32).min(THUMB_SLOT_H / layout_rows as f32);
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
                egui::pos2(
                    row_rect.min.x + 8.0 + THUMB_SLOT_W + 12.0,
                    row_rect.center().y,
                ),
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
                    // text-xs (12px) — Tailwind's smallest type-scale step.
                    egui::FontId::monospace(12.0),
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
            ResizeCorner::TopRight => (
                egui::pos2(state_rect.max.x - r, state_rect.min.y + r),
                -PI / 2.0,
            ),
            ResizeCorner::BottomRight => (state_rect.max - egui::vec2(r, r), 0.0),
            ResizeCorner::BottomLeft => (
                egui::pos2(state_rect.min.x + r, state_rect.max.y - r),
                PI / 2.0,
            ),
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
