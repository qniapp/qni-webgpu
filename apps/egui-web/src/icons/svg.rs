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

pub(super) fn push_cubic_points_viewbox(
    points: &mut Vec<egui::Pos2>,
    rect: egui::Rect,
    p0: SvgPoint,
    p1: SvgPoint,
    p2: SvgPoint,
    p3: SvgPoint,
    steps: usize,
    viewbox: f32,
) {
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let u = 1.0 - t;
        let uu = u * u;
        let tt = t * t;
        let uuu = uu * u;
        let ttt = tt * t;
        let x = uuu * p0.x + 3.0 * uu * t * p1.x + 3.0 * u * tt * p2.x + ttt * p3.x;
        let y = uuu * p0.y + 3.0 * uu * t * p1.y + 3.0 * u * tt * p2.y + ttt * p3.y;
        points.push(map_svg_point_in_rect(rect, SvgPoint::new(x, y), viewbox));
    }
}
