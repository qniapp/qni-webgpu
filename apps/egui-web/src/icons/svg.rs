//! Small helpers for mapping qni SVG viewBox coordinates into egui rects.

use eframe::egui;

#[derive(Clone, Copy)]
pub(super) struct SvgPoint {
    pub(super) x: f32,
    pub(super) y: f32,
}

impl SvgPoint {
    pub(super) fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

pub(super) fn map_svg_point_in_rect(rect: egui::Rect, point: SvgPoint, viewbox: f32) -> egui::Pos2 {
    let scale_x = rect.width() / viewbox;
    let scale_y = rect.height() / viewbox;
    egui::pos2(
        rect.min.x + point.x * scale_x,
        rect.min.y + point.y * scale_y,
    )
}
