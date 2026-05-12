use eframe::egui;

use crate::colors::Colors;

/// Draw the aspect popover (background + rows). Each row shows an
/// aspect-correct thumbnail rect, the cols × rows label, and a "(now)"
/// tag for the current selection. Fixed-height popover with up to
/// qubits+1 rows; for 16 qubits that's 17 rows × 22 px ≈ 374 px,
/// which fits inside `MAX_HEIGHT = 420`.
pub(crate) fn draw_aspect_popover(
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
        color: colors.aspect_popover_shadow,
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
            colors.aspect_thumb_current
        } else {
            colors.aspect_thumb_idle
        };
        painter.rect_filled(
            egui::Rect::from_min_size(thumb_min, egui::vec2(thumb_w, thumb_h)),
            egui::CornerRadius::ZERO,
            thumb_color,
        );
        // Label
        let label = format!("{} × {}", cols, layout_rows);
        let label_color = if is_current {
            colors.aspect_text_current
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
                colors.aspect_text_current,
            );
        }
    }
}
