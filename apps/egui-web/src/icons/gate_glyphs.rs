//! Gate icon glyphs translated from qni SVG primitives.

use eframe::egui;

use crate::colors::Colors;
use crate::gates::GateKind;

use super::bloch::draw_bloch_sphere;
use super::svg::{map_svg_point_in_rect, push_cubic_points_viewbox, SvgPoint};
use super::VIEWBOX;

const CONTROL_RADIUS: f32 = 8.0;
const ANTI_CONTROL_STROKE_WIDTH: f32 = 3.0;
const ANTI_CONTROL_RADIUS: f32 = CONTROL_RADIUS - ANTI_CONTROL_STROKE_WIDTH;

/// Draws the qni meter icon (half-arc + needle + pivot dot) in `color`. Used
/// both by the un-fired gate body and the fired overlay (different color).
pub(crate) fn draw_meter_icon(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let viewbox = VIEWBOX;
    let scale = rect.width() / viewbox;
    let stroke = egui::Stroke::new(2.0 * scale, color);
    let p = |x: f32, y: f32| map_svg_point_in_rect(rect, SvgPoint::new(x, y), viewbox);
    let arc_points: Vec<egui::Pos2> = (0..=24)
        .map(|i| {
            let t = i as f32 / 24.0;
            let angle = std::f32::consts::PI * (1.0 - t);
            let cx = 24.0;
            let cy = 36.0;
            let r = 20.0;
            egui::Pos2::new(cx + r * angle.cos(), cy - r * angle.sin())
        })
        .map(|pos| map_svg_point_in_rect(rect, SvgPoint::new(pos.x, pos.y), viewbox))
        .collect();
    painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
        arc_points, stroke,
    )));
    painter.line_segment([p(24.625, 33.5), p(37.75, 11.0)], stroke);
    // qni's SVG pivot is a 1.875-radius circle with stroke-width=3 outset
    // (≈ 3.4 in viewbox units). Use 3.5*scale to match its visual weight.
    painter.circle_filled(p(24.625, 33.5), 3.5 * scale, color);
}

pub(super) fn draw_gate_icon(
    painter: &egui::Painter,
    rect: egui::Rect,
    kind: GateKind,
    color: egui::Color32,
    colors: &Colors,
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
        GateKind::BlochDisplay => {
            // Stand-alone sphere with crossed axes drawn directly on the wire,
            // matching qni's bloch-display element. The dynamic Bloch vector is
            // overlaid by `render::draw_bloch_display` so it can read the
            // current state.
            draw_bloch_sphere(painter, rect, color);
            true
        }
        GateKind::Measurement => {
            // qni reference: packages/elements/icon/measurement-gate.svg
            draw_meter_icon(painter, rect, color);
            true
        }
        GateKind::Spacer => {
            // qni reference: packages/elements/icon/spacer-gate.svg
            // Three filled 6×6 squares at x=9, 21, 33 (y=21–27 in viewbox).
            let rect_at = |x: f32| {
                egui::Rect::from_min_max(
                    map_svg_point_in_rect(rect, SvgPoint::new(x, 21.0), viewbox),
                    map_svg_point_in_rect(rect, SvgPoint::new(x + 6.0, 27.0), viewbox),
                )
            };
            painter.rect_filled(rect_at(9.0), egui::CornerRadius::ZERO, color);
            painter.rect_filled(rect_at(21.0), egui::CornerRadius::ZERO, color);
            painter.rect_filled(rect_at(33.0), egui::CornerRadius::ZERO, color);
            true
        }
        GateKind::Write0 | GateKind::Write1 => {
            // qni reference: packages/elements/icon/write-gate.svg
            painter.line_segment([p(6.0, 5.0), p(6.0, 43.0)], stroke);
            painter.line_segment([p(37.4516, 5.0), p(43.5, 24.0)], stroke);
            painter.line_segment([p(43.5, 24.0), p(37.4516, 43.0)], stroke);
            let (digit, digit_color) = if kind == GateKind::Write0 {
                ("0", colors.semantic_off)
            } else {
                ("1", colors.semantic_on)
            };
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                digit,
                egui::FontId::monospace(16.0),
                digit_color,
            );
            true
        }
        GateKind::Control => {
            painter.circle_filled(p(24.0, 24.0), CONTROL_RADIUS * scale, color);
            true
        }
        GateKind::AntiControl => {
            let anti_control_stroke = egui::Stroke::new(ANTI_CONTROL_STROKE_WIDTH * scale, color);
            painter.circle_stroke(
                p(24.0, 24.0),
                ANTI_CONTROL_RADIUS * scale,
                anti_control_stroke,
            );
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
        // QFT family draws its label via the special body branch above,
        // not through this icon table.
        GateKind::QftGate | GateKind::QftDaggerGate => false,
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
