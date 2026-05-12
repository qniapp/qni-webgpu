//! State panel hover popup and aspect-ratio popover drawing.

use eframe::egui;
use eframe::egui_wgpu;

use super::state_panel_layout::StatePanelLayout;
use crate::colors::Colors;
use crate::gpu::{PopupValueCallback, POPUP_GLYPH_CELL_H, POPUP_GLYPH_CELL_W};

/// Hover popup showing the |ket⟩ + amplitude / probability / phase for
/// the cell under the pointer. Paper background + ui-2 1px border +
/// soft shadow (B variant from `docs/state-cell-popup-mockups.html`),
/// with qni-style icons rendered as egui primitives.
///
/// Numeric amplitude / probability / phase values are rendered by
/// `PopupValueCallback` on the GPU. The CPU computes only layout,
/// colours, and glyph atlas placement; it performs no value readback.
pub(super) fn draw_state_cell_popup(
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
    let popup_h =
        POPUP_PAD_Y * 2.0 + HEADER_TEXT_H + HEADER_GAP + ROW_H * (ROWS as f32 - 1.0) + BODY_TEXT_H;
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
            0 => draw_amplitude_icon(painter, icon_rect, icon_chrome, icon_accent),
            1 => draw_probability_icon(painter, icon_rect, icon_chrome, icon_accent),
            _ => draw_phase_icon(painter, icon_rect, icon_chrome, icon_accent),
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
    let paint_callback = egui_wgpu::Callback::new_paint_callback(screen_rect, popup_value_callback);
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
        let slot_rect = egui::Rect::from_min_size(slot_min, egui::vec2(THUMB_SLOT_W, THUMB_SLOT_H));
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
