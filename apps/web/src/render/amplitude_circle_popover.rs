use eframe::egui;

use crate::gpu::{POPUP_GLYPH_CELL_H, POPUP_GLYPH_CELL_W};

use super::popover;

pub(crate) const WIDTH: f32 = 296.0;
pub(crate) const PAD_X: f32 = crate::tailwind::SPACING_4;
pub(crate) const PAD_Y: f32 = crate::tailwind::SPACING_3;
pub(crate) const HEADER_H: f32 = crate::tailwind::LEADING_5;
pub(crate) const HEADER_GAP: f32 = crate::tailwind::SPACING_2;
pub(crate) const ROW_H: f32 = crate::tailwind::LEADING_5;
pub(crate) const BODY_TEXT_H: f32 = crate::tailwind::LEADING_4;
pub(crate) const ICON_SIZE: f32 = crate::tailwind::SPACING_4;
pub(crate) const ICON_TEXT_GAP: f32 = crate::tailwind::SPACING_2;
pub(crate) const LABEL_W: f32 = 88.0;
pub(crate) const VALUE_CHARS: f32 = 17.0;
pub(crate) const ROWS: usize = 3;

pub(crate) fn height() -> f32 {
    PAD_Y * 2.0 + HEADER_H + HEADER_GAP + ROW_H * (ROWS as f32 - 1.0) + BODY_TEXT_H
}

pub(crate) fn header_pos(rect: egui::Rect) -> egui::Pos2 {
    egui::pos2(rect.left() + PAD_X, rect.top() + PAD_Y)
}

pub(crate) fn row_y(rect: egui::Rect, row: usize) -> f32 {
    rect.top() + PAD_Y + HEADER_H + HEADER_GAP + row as f32 * ROW_H
}

pub(crate) fn icon_rect(rect: egui::Rect, row: usize) -> egui::Rect {
    egui::Rect::from_min_size(
        egui::pos2(
            rect.left() + PAD_X,
            row_y(rect, row) + (ROW_H - ICON_SIZE) * 0.5,
        ),
        egui::vec2(ICON_SIZE, ICON_SIZE),
    )
}

pub(crate) fn label_pos(rect: egui::Rect, row: usize) -> egui::Pos2 {
    let icon_rect = icon_rect(rect, row);
    egui::pos2(icon_rect.right() + ICON_TEXT_GAP, row_y(rect, row))
}

pub(crate) fn value_anchor(rect: egui::Rect) -> egui::Pos2 {
    egui::pos2(
        rect.left() + PAD_X + ICON_SIZE + ICON_TEXT_GAP + LABEL_W + ICON_TEXT_GAP,
        row_y(rect, 0) - 2.0,
    )
}

pub(crate) fn value_clip_rect(value_anchor: egui::Pos2) -> egui::Rect {
    egui::Rect::from_min_size(
        value_anchor,
        egui::vec2(POPUP_GLYPH_CELL_W as f32 * VALUE_CHARS, ROW_H * ROWS as f32),
    )
}

pub(crate) fn popup_glyph_char_size() -> [f32; 2] {
    [POPUP_GLYPH_CELL_W as f32, POPUP_GLYPH_CELL_H as f32]
}

pub(crate) fn clamp_horizontally_to_bounds(
    mut rect: egui::Rect,
    bounds_rect: egui::Rect,
) -> egui::Rect {
    let min_left = bounds_rect.left() + popover::VIEWPORT_PAD;
    let max_left = bounds_rect.right() - popover::VIEWPORT_PAD - rect.width();
    let target_left = if max_left >= min_left {
        rect.left().clamp(min_left, max_left)
    } else {
        min_left
    };
    rect = rect.translate(egui::vec2(target_left - rect.left(), 0.0));
    rect
}

pub(crate) fn outline_radius_for_cell(cell: f32) -> f32 {
    let stroke = if cell >= 24.0 { 2.0 } else { 1.0 };
    let outline_clearance = 1.5;
    (cell * 0.5 - stroke * 0.5 - outline_clearance).max(0.0)
}
