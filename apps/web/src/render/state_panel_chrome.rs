use eframe::egui;

use super::state_panel_layout::StatePanelLayout;
use crate::app::ResizeCorner;
use crate::colors::Colors;
use crate::constants::{
    STATE_PANEL_CORNER_RADIUS, STATE_RESIZE_HANDLE_PAD, STATE_RESIZE_HANDLE_STROKE,
};

pub(super) fn draw_resize_handle_arc(
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

pub(super) fn draw_state_minimap(
    painter: &egui::Painter,
    colors: &Colors,
    layout: &StatePanelLayout,
    viewport_rect: egui::Rect,
    grid_origin: egui::Pos2,
) {
    let grid = layout.grid_size;
    if grid.x <= viewport_rect.width() && grid.y <= viewport_rect.height() {
        return;
    }
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
    let pad = 12.0;
    let mm_rect = egui::Rect::from_min_max(
        viewport_rect.max - mm_size - egui::vec2(pad, pad),
        viewport_rect.max - egui::vec2(pad, pad),
    );
    painter.rect_filled(mm_rect, egui::CornerRadius::same(4), colors.minimap_bg);

    let mm_grid_min = mm_rect.min + egui::vec2(INSET, INSET);
    let mm_grid_size = egui::vec2(inner_w, inner_h);
    let scale = inner_w / grid.x;
    let visible_offset = viewport_rect.min - grid_origin;
    let mm_vp_min = mm_grid_min + visible_offset * scale;
    let mm_vp_size = egui::vec2(viewport_rect.width(), viewport_rect.height()) * scale;
    let mm_vp_rect = egui::Rect::from_min_size(mm_vp_min, mm_vp_size)
        .intersect(egui::Rect::from_min_size(mm_grid_min, mm_grid_size));
    let vp_fill = colors.minimap_viewport_fill;
    let vp_stroke = egui::Stroke::new(1.0_f32, colors.minimap_viewport_stroke);
    painter.rect_filled(mm_vp_rect, egui::CornerRadius::ZERO, vp_fill);
    painter.rect_stroke(
        mm_vp_rect,
        egui::CornerRadius::ZERO,
        vp_stroke,
        egui::StrokeKind::Inside,
    );
}
