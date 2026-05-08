use eframe::egui;

use crate::colors::Colors;
use crate::gates::GateKind;

const VIEWBOX: f32 = 48.0;

#[derive(Clone, Copy)]
struct SvgPoint {
    x: f32,
    y: f32,
}

impl SvgPoint {
    fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

fn map_svg_point_in_rect(rect: egui::Rect, point: SvgPoint, viewbox: f32) -> egui::Pos2 {
    let scale_x = rect.width() / viewbox;
    let scale_y = rect.height() / viewbox;
    egui::pos2(
        rect.min.x + point.x * scale_x,
        rect.min.y + point.y * scale_y,
    )
}

fn push_cubic_points_viewbox(
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

pub(crate) fn draw_gate_body(
    painter: &egui::Painter,
    gate_rect: egui::Rect,
    kind: GateKind,
    colors: &Colors,
) {
    draw_gate_body_with_fill(painter, gate_rect, kind, colors, colors.box_fill);
}

pub(crate) fn draw_drag_gate_body(
    painter: &egui::Painter,
    gate_rect: egui::Rect,
    kind: GateKind,
    colors: &Colors,
) {
    draw_gate_body_with_fill(painter, gate_rect, kind, colors, colors.drag_fill);
}

fn draw_gate_body_with_fill(
    painter: &egui::Painter,
    gate_rect: egui::Rect,
    kind: GateKind,
    colors: &Colors,
    fill: egui::Color32,
) {
    if kind == GateKind::X {
        let radius = gate_rect.width().min(gate_rect.height()) / 2.0;
        painter.circle_filled(gate_rect.center(), radius, fill);
    } else if kind != GateKind::Control && kind != GateKind::Swap {
        painter.rect_filled(gate_rect, egui::CornerRadius::same(6), fill);
    }
    let icon_color = if kind == GateKind::Control || kind == GateKind::Swap {
        fill
    } else {
        colors.label
    };
    if !draw_gate_icon(painter, gate_rect, kind, icon_color) {
        painter.text(
            gate_rect.center(),
            egui::Align2::CENTER_CENTER,
            kind.label(),
            egui::FontId::proportional(18.0),
            colors.label,
        );
    }
}

fn draw_gate_icon(
    painter: &egui::Painter,
    rect: egui::Rect,
    kind: GateKind,
    color: egui::Color32,
) -> bool {
    let viewbox = VIEWBOX;
    let scale = rect.width() / viewbox;
    let stroke = egui::Stroke::new(2.0 * scale, color);
    let p = |x: f32, y: f32| map_svg_point_in_rect(rect, SvgPoint::new(x, y), viewbox);

    match kind {
        GateKind::H => {
            painter.line_segment([p(17.0, 13.0), p(17.0, 35.0)], stroke);
            painter.line_segment([p(17.0, 24.0), p(31.0, 24.0)], stroke);
            painter.line_segment([p(31.0, 13.0), p(31.0, 35.0)], stroke);
            true
        }
        GateKind::Control => {
            painter.circle_filled(p(24.0, 24.0), 8.0 * scale, color);
            true
        }
        GateKind::X => {
            painter.line_segment([p(15.0, 24.0), p(33.0, 24.0)], stroke);
            painter.line_segment([p(24.0, 15.0), p(24.0, 33.0)], stroke);
            true
        }
        GateKind::Y => {
            painter.line_segment([p(17.0, 13.0), p(24.0, 24.0)], stroke);
            painter.line_segment([p(24.0, 24.0), p(31.0, 13.0)], stroke);
            painter.line_segment([p(24.0, 24.0), p(24.0, 35.0)], stroke);
            true
        }
        GateKind::Z => {
            let points = vec![p(17.5, 13.0), p(31.0, 13.0), p(17.5, 35.0), p(31.0, 35.0)];
            painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
                points, stroke,
            )));
            true
        }
        GateKind::S => {
            draw_s_curve(painter, rect, stroke);
            true
        }
        GateKind::SDagger => {
            draw_s_curve(painter, rect, stroke);
            painter.line_segment([p(37.0, 10.0), p(43.0, 10.0)], stroke);
            painter.line_segment([p(40.0, 6.0), p(40.0, 20.0)], stroke);
            true
        }
        GateKind::T => {
            painter.line_segment([p(15.0, 13.0), p(33.0, 13.0)], stroke);
            painter.line_segment([p(24.0, 13.0), p(24.0, 35.0)], stroke);
            true
        }
        GateKind::TDagger => {
            painter.line_segment([p(15.0, 13.0), p(33.0, 13.0)], stroke);
            painter.line_segment([p(24.0, 13.0), p(24.0, 35.0)], stroke);
            painter.line_segment([p(37.0, 10.0), p(43.0, 10.0)], stroke);
            painter.line_segment([p(40.0, 6.0), p(40.0, 20.0)], stroke);
            true
        }
        GateKind::SqrtX => {
            let points = vec![
                p(10.0, 24.0),
                p(13.0, 24.0),
                p(14.0, 36.0),
                p(17.0, 36.0),
                p(18.0, 12.0),
                p(38.0, 12.0),
            ];
            painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
                points, stroke,
            )));
            painter.line_segment([p(24.0, 32.0), p(34.0, 18.0)], stroke);
            painter.line_segment([p(34.0, 32.0), p(24.0, 18.0)], stroke);
            true
        }
        GateKind::Swap => {
            let scale = rect.width() / VIEWBOX;
            let swap_stroke = egui::Stroke::new(4.0 * scale, color);
            painter.line_segment([p(12.0, 36.0), p(36.0, 12.0)], swap_stroke);
            painter.line_segment([p(12.0, 12.0), p(36.0, 36.0)], swap_stroke);
            true
        }
        GateKind::Phase => {
            painter.line_segment([p(18.2857, 36.0), p(29.7143, 12.0)], stroke);
            painter.circle_stroke(p(24.0, 24.5714), 8.0 * scale, stroke);
            true
        }
        GateKind::Rx => {
            draw_r_letter(painter, rect, stroke);
            painter.line_segment([p(34.6093, 13.0016), p(24.7475, 35.0)], stroke);
            painter.line_segment([p(24.8187, 13.0016), p(34.6093, 35.0)], stroke);
            true
        }
        GateKind::Ry => {
            draw_r_letter(painter, rect, stroke);
            painter.line_segment([p(34.6093, 13.0016), p(29.5, 23.5)], stroke);
            painter.line_segment([p(29.5, 23.5), p(29.5, 35.0)], stroke);
            painter.line_segment([p(24.5, 13.0), p(29.5, 23.5)], stroke);
            true
        }
        GateKind::Rz => {
            draw_r_letter(painter, rect, stroke);
            let points = vec![p(24.5, 13.0), p(34.5, 13.0), p(24.5, 35.0), p(34.5, 35.0)];
            painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
                points, stroke,
            )));
            true
        }
    }
}

fn draw_r_letter(painter: &egui::Painter, rect: egui::Rect, stroke: egui::Stroke) {
    let viewbox = VIEWBOX;
    let p = |x: f32, y: f32| map_svg_point_in_rect(rect, SvgPoint::new(x, y), viewbox);
    painter.line_segment([p(12.3214, 35.0), p(12.3214, 24.0)], stroke);
    painter.line_segment([p(18.0, 24.5), p(21.7303, 35.0)], stroke);

    let mut points = Vec::new();
    let start = SvgPoint::new(12.3214, 24.0);
    points.push(p(start.x, start.y));
    points.push(p(12.3214, 13.0));
    push_cubic_points_viewbox(
        &mut points,
        rect,
        SvgPoint::new(12.3214, 13.0),
        SvgPoint::new(21.0, 13.0),
        SvgPoint::new(22.0, 15.5),
        SvgPoint::new(22.0, 18.5),
        10,
        viewbox,
    );
    push_cubic_points_viewbox(
        &mut points,
        rect,
        SvgPoint::new(22.0, 18.5),
        SvgPoint::new(22.0, 21.5),
        SvgPoint::new(21.0, 24.0),
        SvgPoint::new(12.3214, 24.0),
        10,
        viewbox,
    );
    painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
        points, stroke,
    )));
}

fn draw_s_curve(painter: &egui::Painter, rect: egui::Rect, stroke: egui::Stroke) {
    let viewbox = VIEWBOX;
    let mut points = Vec::new();
    let start = SvgPoint::new(30.0, 15.5982);
    points.push(map_svg_point_in_rect(rect, start, viewbox));

    push_cubic_points_viewbox(
        &mut points,
        rect,
        start,
        SvgPoint::new(30.0, 15.5982),
        SvgPoint::new(29.0, 13.5893),
        SvgPoint::new(25.0, 13.3512),
        12,
        viewbox,
    );
    push_cubic_points_viewbox(
        &mut points,
        rect,
        SvgPoint::new(25.0, 13.3512),
        SvgPoint::new(21.5, 13.1429),
        SvgPoint::new(16.5, 13.8029),
        SvgPoint::new(17.0, 19.1515),
        12,
        viewbox,
    );
    push_cubic_points_viewbox(
        &mut points,
        rect,
        SvgPoint::new(17.0, 19.1515),
        SvgPoint::new(17.5, 24.5001),
        SvgPoint::new(31.0, 23.1432),
        SvgPoint::new(31.0, 29.035),
        12,
        viewbox,
    );
    push_cubic_points_viewbox(
        &mut points,
        rect,
        SvgPoint::new(31.0, 29.035),
        SvgPoint::new(31.0, 34.9268),
        SvgPoint::new(25.5934, 35.2343),
        SvgPoint::new(21.5, 34.9268),
        12,
        viewbox,
    );
    push_cubic_points_viewbox(
        &mut points,
        rect,
        SvgPoint::new(21.5, 34.9268),
        SvgPoint::new(19.0063, 34.7396),
        SvgPoint::new(17.0, 33.2578),
        SvgPoint::new(17.0, 33.2578),
        10,
        viewbox,
    );

    painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
        points, stroke,
    )));
}
