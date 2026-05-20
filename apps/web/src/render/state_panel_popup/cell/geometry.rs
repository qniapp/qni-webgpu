use eframe::egui;

use crate::gpu::{POPUP_GLYPH_CELL_H, POPUP_GLYPH_CELL_W};
use crate::render::state_panel_layout::StatePanelLayout;

// Popup geometry, on the Tailwind 4-px spacing scale. Heights are
// derived from each text size's default Tailwind line-height so the gap
// above the header and below the last row match exactly.
//   POPUP_W       — header + body width (no Tailwind preset for "tooltip
//                   width"; sized to fit the widest row, 17 chars × 9 px
//                   glyph cell + chrome).
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
const TAIL_H: f32 = 8.0;
const TAIL_HALF_W: f32 = 8.0;
const GAP_TO_CELL: f32 = 4.0;
const SCREEN_TOP_MARGIN: f32 = 8.0;
const ICON_SIZE: f32 = 16.0;
const ICON_TEXT_GAP: f32 = 8.0;
const LABEL_X_OFFSET: f32 = ICON_SIZE + ICON_TEXT_GAP;
const VALUE_X_OFFSET: f32 = LABEL_X_OFFSET + 96.0;

pub(super) struct PopupGeometry {
    pub(super) rect: egui::Rect,
    cell_center: egui::Pos2,
    tail_apex_y: f32,
    tail_base_y: f32,
}

impl PopupGeometry {
    pub(super) fn tail_apex(&self) -> egui::Pos2 {
        egui::pos2(self.cell_center.x, self.tail_apex_y)
    }

    pub(super) fn tail_base_l(&self) -> egui::Pos2 {
        egui::pos2(self.cell_center.x - TAIL_HALF_W, self.tail_base_y)
    }

    pub(super) fn tail_base_r(&self) -> egui::Pos2 {
        egui::pos2(self.cell_center.x + TAIL_HALF_W, self.tail_base_y)
    }

    pub(super) fn header_y(&self) -> f32 {
        self.rect.min.y + POPUP_PAD_Y
    }

    pub(super) fn header_pos(&self) -> egui::Pos2 {
        egui::pos2(self.rect.min.x + POPUP_PAD_X, self.header_y())
    }

    pub(super) fn row_y(&self, row: usize) -> f32 {
        self.header_y() + HEADER_TEXT_H + HEADER_GAP + row as f32 * ROW_H
    }

    pub(super) fn icon_rect(&self, row: usize) -> egui::Rect {
        let y = self.row_y(row);
        egui::Rect::from_min_size(
            egui::pos2(self.rect.min.x + POPUP_PAD_X, y + (ROW_H - ICON_SIZE) * 0.5),
            egui::vec2(ICON_SIZE, ICON_SIZE),
        )
    }

    pub(super) fn label_pos(&self, row: usize) -> egui::Pos2 {
        egui::pos2(
            self.rect.min.x + POPUP_PAD_X + LABEL_X_OFFSET,
            self.row_y(row),
        )
    }

    pub(super) fn value_anchor(&self) -> egui::Pos2 {
        egui::pos2(
            self.rect.min.x + POPUP_PAD_X + VALUE_X_OFFSET,
            // Egui draws monospace text with the cap-height a touch below the
            // cell top; nudge the atlas anchor up so the digits sit on the
            // same baseline as the labels to their left.
            self.row_y(0) - 2.0,
        )
    }

    pub(super) fn value_clip_rect(&self) -> egui::Rect {
        egui::Rect::from_min_size(
            self.value_anchor(),
            egui::vec2(
                POPUP_GLYPH_CELL_W as f32 * 17.0, // widest row = amplitude (17 chars)
                ROW_H * ROWS as f32,
            ),
        )
    }

    pub(super) fn row_pitch(&self) -> f32 {
        ROW_H
    }
}

pub(super) fn layout_popup(
    layout: &StatePanelLayout,
    grid_origin: egui::Pos2,
    viewport_rect: egui::Rect,
    display_index: u32,
) -> PopupGeometry {
    let cell_center = cell_center_for(layout, grid_origin, display_index);
    let popup_h = popup_height();

    // Prefer above the cell — the state panel anchors to the bottom of the
    // screen, so "above" usually lands in the empty page area. Only flip below
    // if going above would push the popup past the screen top.
    let above_top = cell_center.y - layout.radius - GAP_TO_CELL - TAIL_H - popup_h;
    let prefer_above = above_top >= SCREEN_TOP_MARGIN;
    let (popup_rect, tail_apex_y, tail_base_y) = if prefer_above {
        let rect = egui::Rect::from_min_size(
            egui::pos2(cell_center.x - POPUP_W * 0.5, above_top),
            egui::vec2(POPUP_W, popup_h),
        );
        (rect, rect.max.y + TAIL_H, rect.max.y)
    } else {
        let top = cell_center.y + layout.radius + GAP_TO_CELL + TAIL_H;
        let rect = egui::Rect::from_min_size(
            egui::pos2(cell_center.x - POPUP_W * 0.5, top),
            egui::vec2(POPUP_W, popup_h),
        );
        (rect, rect.min.y - TAIL_H, rect.min.y)
    };

    PopupGeometry {
        rect: clamp_horizontally_to_viewport(popup_rect, viewport_rect),
        cell_center,
        tail_apex_y,
        tail_base_y,
    }
}

fn cell_center_for(
    layout: &StatePanelLayout,
    grid_origin: egui::Pos2,
    display_index: u32,
) -> egui::Pos2 {
    let pitch = layout.cell_pitch();
    let cols = layout.columns().max(1);
    let col = (display_index as usize) % cols;
    let row = (display_index as usize) / cols;
    egui::pos2(
        grid_origin.x + col as f32 * pitch + pitch * 0.5,
        grid_origin.y + row as f32 * pitch + pitch * 0.5,
    )
}

fn popup_height() -> f32 {
    POPUP_PAD_Y * 2.0 + HEADER_TEXT_H + HEADER_GAP + ROW_H * (ROWS as f32 - 1.0) + BODY_TEXT_H
}

fn clamp_horizontally_to_viewport(mut rect: egui::Rect, viewport_rect: egui::Rect) -> egui::Rect {
    if rect.min.x < viewport_rect.min.x + 4.0 {
        let dx = viewport_rect.min.x + 4.0 - rect.min.x;
        rect = rect.translate(egui::vec2(dx, 0.0));
    } else if rect.max.x > viewport_rect.max.x - 4.0 {
        let dx = viewport_rect.max.x - 4.0 - rect.max.x;
        rect = rect.translate(egui::vec2(dx, 0.0));
    }
    rect
}

pub(super) fn popup_glyph_char_size() -> [f32; 2] {
    [POPUP_GLYPH_CELL_W as f32, POPUP_GLYPH_CELL_H as f32]
}
