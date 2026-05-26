use eframe::egui;

use crate::render::amplitude_circle_popover as amplitude_popover;
use crate::render::popover;
use crate::render::state_panel_layout::StatePanelLayout;

pub(super) struct PopupGeometry {
    pub(super) rect: egui::Rect,
    cell_center: egui::Pos2,
    tail_base_center_x: f32,
    tail_apex_y: f32,
    tail_base_y: f32,
}

impl PopupGeometry {
    pub(super) fn tail_apex(&self) -> egui::Pos2 {
        egui::pos2(self.cell_center.x, self.tail_apex_y)
    }

    pub(super) fn tail_base_l(&self) -> egui::Pos2 {
        egui::pos2(
            self.tail_base_center_x - popover::TAIL_HALF_W,
            self.tail_base_y,
        )
    }

    pub(super) fn tail_base_r(&self) -> egui::Pos2 {
        egui::pos2(
            self.tail_base_center_x + popover::TAIL_HALF_W,
            self.tail_base_y,
        )
    }

    pub(super) fn header_pos(&self) -> egui::Pos2 {
        amplitude_popover::header_pos(self.rect)
    }

    pub(super) fn icon_rect(&self, row: usize) -> egui::Rect {
        amplitude_popover::icon_rect(self.rect, row)
    }

    pub(super) fn label_pos(&self, row: usize) -> egui::Pos2 {
        amplitude_popover::label_pos(self.rect, row)
    }

    pub(super) fn value_anchor(&self) -> egui::Pos2 {
        amplitude_popover::value_anchor(self.rect)
    }

    pub(super) fn value_clip_rect(&self) -> egui::Rect {
        amplitude_popover::value_clip_rect(self.value_anchor())
    }

    pub(super) fn row_pitch(&self) -> f32 {
        amplitude_popover::ROW_H
    }
}

pub(super) fn layout_popup(
    layout: &StatePanelLayout,
    grid_origin: egui::Pos2,
    bounds_rect: egui::Rect,
    display_index: u32,
) -> PopupGeometry {
    let cell_center = cell_center_for(layout, grid_origin, display_index);
    let popup_h = amplitude_popover::height();

    // Prefer above the cell — the state panel anchors to the bottom of the
    // screen, so "above" usually lands in the empty page area. Only flip below
    // if going above would push the popup past the viewport padding.
    let above_top = cell_center.y - layout.radius - popover::ANCHOR_GAP - popover::TAIL_H - popup_h;
    let prefer_above = above_top >= bounds_rect.top() + popover::VIEWPORT_PAD;
    let (popup_rect, tail_apex_y, tail_base_y) = if prefer_above {
        let rect = egui::Rect::from_min_size(
            egui::pos2(cell_center.x - amplitude_popover::WIDTH * 0.5, above_top),
            egui::vec2(amplitude_popover::WIDTH, popup_h),
        );
        (rect, rect.max.y + popover::TAIL_H, rect.max.y)
    } else {
        let top = cell_center.y + layout.radius + popover::ANCHOR_GAP + popover::TAIL_H;
        let rect = egui::Rect::from_min_size(
            egui::pos2(cell_center.x - amplitude_popover::WIDTH * 0.5, top),
            egui::vec2(amplitude_popover::WIDTH, popup_h),
        );
        (rect, rect.min.y - popover::TAIL_H, rect.min.y)
    };

    let rect = amplitude_popover::clamp_horizontally_to_bounds(popup_rect, bounds_rect);
    let tail_base_center_x = cell_center.x.clamp(
        rect.min.x + popover::TAIL_HALF_W,
        rect.max.x - popover::TAIL_HALF_W,
    );

    PopupGeometry {
        rect,
        cell_center,
        tail_base_center_x,
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

pub(super) fn popup_glyph_char_size() -> [f32; 2] {
    amplitude_popover::popup_glyph_char_size()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_layout() -> StatePanelLayout {
        StatePanelLayout {
            state_count: 1,
            qubits: 1,
            columns: 1,
            size: 64.0,
            gap: 3.0,
            radius: 32.0,
            stroke: 2.0,
            inner_radius: 31.0,
            grid_size: egui::vec2(64.0, 64.0),
            viewport_rect: egui::Rect::from_min_size(
                egui::pos2(300.0, 300.0),
                egui::vec2(560.0, 160.0),
            ),
            state_rect: egui::Rect::from_min_size(
                egui::pos2(300.0, 268.0),
                egui::vec2(560.0, 192.0),
            ),
            handle_height: 32.0,
        }
    }

    #[test]
    fn left_edge_popup_body_stays_under_tail_base() {
        let popup = layout_popup(
            &test_layout(),
            egui::pos2(0.0, 240.0),
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)),
            0,
        );

        assert!(popup.rect.min.x <= popup.tail_base_l().x);
    }

    #[test]
    fn right_edge_popup_body_stays_under_tail_base() {
        let popup = layout_popup(
            &test_layout(),
            egui::pos2(736.0, 240.0),
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)),
            0,
        );

        assert!(popup.rect.max.x >= popup.tail_base_r().x);
    }

    #[test]
    fn screen_left_edge_tail_base_clamps_to_body() {
        let popup = layout_popup(
            &test_layout(),
            egui::pos2(-24.0, 240.0),
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)),
            0,
        );

        assert!((popup.tail_base_l().x - popup.rect.min.x).abs() < 0.001);
    }

    #[test]
    fn screen_right_edge_tail_base_clamps_to_body() {
        let popup = layout_popup(
            &test_layout(),
            egui::pos2(760.0, 240.0),
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)),
            0,
        );

        assert!((popup.tail_base_r().x - popup.rect.max.x).abs() < 0.001);
    }
}
