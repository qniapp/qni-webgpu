use eframe::egui;

use crate::colors::Colors;
use crate::gates::GateKind;
use crate::tailwind::{
    SPACING_1, SPACING_10, SPACING_2, SPACING_3, SPACING_5, SPACING_6, SPACING_9, STROKE_2,
    TEXT_BASE,
};

use super::bloch_display::draw_bloch_sphere;
use super::svg::{map_svg_point_in_rect, SvgPoint};
use super::VIEWBOX;

const CONTROL_RADIUS: f32 = 8.0;
const ANTI_CONTROL_STROKE_WIDTH: f32 = 3.0;
const ANTI_CONTROL_RADIUS: f32 = CONTROL_RADIUS - ANTI_CONTROL_STROKE_WIDTH;
const DETECTOR_CONTROL_RADIUS: f32 = 5.0;
const DETECTOR_DIGIT_SCALE: f32 = 0.7;

pub(crate) fn draw_meter_icon(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let viewbox = VIEWBOX;
    let scale = rect.width() / viewbox;
    let stroke = egui::Stroke::new(2.0 * scale, color);
    let p = |x: f32, y: f32| map_svg_point_in_rect(rect, SvgPoint::new(x, y), viewbox);
    draw_meter_arc(painter, rect, SvgPoint::new(24.0, 36.0), 20.0, stroke);
    painter.line_segment([p(24.625, 33.5), p(37.75, 11.0)], stroke);
    // qni's SVG pivot is a 1.875-radius circle with stroke-width=3 outset
    // (≈ 3.4 in viewbox units). Use 3.5*scale to match its visual weight.
    painter.circle_filled(p(24.625, 33.5), 3.5 * scale, color);
}

fn draw_meter_arc(
    painter: &egui::Painter,
    rect: egui::Rect,
    center: SvgPoint,
    radius: f32,
    stroke: egui::Stroke,
) {
    let points = (0..=24)
        .map(|i| {
            let angle = std::f32::consts::PI * (1.0 - i as f32 / 24.0);
            SvgPoint::new(
                center.x + radius * angle.cos(),
                center.y - radius * angle.sin(),
            )
        })
        .map(|point| map_svg_point_in_rect(rect, point, VIEWBOX))
        .collect();
    painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
        points, stroke,
    )));
}

pub(crate) fn draw_sampled_measurement_icon(
    painter: &egui::Painter,
    rect: egui::Rect,
    kind: GateKind,
    color: egui::Color32,
    axis_color: egui::Color32,
) {
    draw_meter_icon(painter, rect, color);
    let axis = match kind {
        GateKind::XDetector => "X",
        GateKind::YDetector => "Y",
        _ => return,
    };
    painter.text(
        rect.left_top() + egui::vec2(crate::tailwind::SPACING_1, 0.0),
        egui::Align2::LEFT_TOP,
        axis,
        egui::FontId::proportional(crate::tailwind::TEXT_XS),
        axis_color,
    );
}

pub(super) fn draw_gate_icon(
    painter: &egui::Painter,
    rect: egui::Rect,
    kind: GateKind,
    color: egui::Color32,
    colors: &Colors,
    grabbed: bool,
) {
    // H / Y / Z / √X / S / S† / T / T† / Phase(Φ) / RX / RY / RZ / QFT /
    // QFT† / X (本体は "+" として描く) / Write の 0・1 は
    // `assets/icons/{h,y,z,sqrtx,s,sdagger,t,tdagger,p,rx,ry,rz,qft,qftdagger,plus,digit0,digit1}.svg`
    // を 1 ソースに、ビルド時に 256 px PNG から作った SDF テクスチャで
    // 描画する。パレットと回路のサブピクセル位置差で見かけの太さが揺れず、
    // 拡大時も輪郭をシェーダで再構成できるようにする。
    if let Some(glyph) = svg_glyph_for(kind) {
        super::svg_icon::draw_glyph(painter, rect, color, glyph);
        return;
    }

    let viewbox = VIEWBOX;
    let scale = rect.width() / viewbox;
    let stroke = egui::Stroke::new(2.0 * scale, color);
    let p = |x: f32, y: f32| map_svg_point_in_rect(rect, SvgPoint::new(x, y), viewbox);

    match kind {
        GateKind::Increment => {
            draw_increment_icon(painter, rect, color);
        }
        GateKind::Decrement => {
            draw_decrement_icon(painter, rect, color);
        }
        GateKind::PostSelectOff
        | GateKind::PostSelectOn
        | GateKind::PostSelectAntiX
        | GateKind::PostSelectX => {
            let (symbol, symbol_color) = match kind {
                GateKind::PostSelectOff => ("0", colors.semantic_off),
                GateKind::PostSelectOn => ("1", colors.semantic_on),
                GateKind::PostSelectAntiX => ("+", colors.semantic_intermediate),
                _ => ("−", colors.semantic_intermediate),
            };
            let symbol_color = if grabbed { colors.label } else { symbol_color };
            let projector_stroke = egui::Stroke::new(STROKE_2 * scale, color);
            for x in [SPACING_1, VIEWBOX - SPACING_1] {
                painter.line_segment([p(x, SPACING_2), p(x, SPACING_10)], projector_stroke);
            }
            for x in [SPACING_5, VIEWBOX - SPACING_5] {
                painter.line_segment([p(x, SPACING_2), p(SPACING_6, SPACING_6)], projector_stroke);
                painter.line_segment(
                    [p(SPACING_6, SPACING_6), p(x, SPACING_10)],
                    projector_stroke,
                );
            }
            for x in [SPACING_3, SPACING_9] {
                painter.text(
                    p(x, SPACING_6),
                    egui::Align2::CENTER_CENTER,
                    symbol,
                    egui::FontId::monospace(TEXT_BASE * scale),
                    symbol_color,
                );
            }
        }
        GateKind::BlochDisplay => {
            draw_bloch_sphere(painter, rect, color);
        }
        GateKind::ZDetectControlReset => {
            let (meter_color, ket_color, digit_color) = if grabbed {
                (colors.drag_accent, colors.drag_accent, colors.label)
            } else {
                (
                    colors.semantic_intermediate,
                    colors.semantic_disabled,
                    colors.semantic_off,
                )
            };
            let meter_stroke = egui::Stroke::new(stroke.width, meter_color);
            let ket_stroke = egui::Stroke::new(stroke.width, ket_color);
            draw_meter_arc(painter, rect, SvgPoint::new(24.0, 24.0), 18.0, meter_stroke);
            painter.line_segment([p(24.0, 24.0), p(35.8, 3.75)], meter_stroke);
            painter.line_segment([p(24.0, 24.0), p(24.0, 45.0)], stroke);
            painter.line_segment([p(40.0, 28.0), p(43.5, 36.5)], ket_stroke);
            painter.line_segment([p(43.5, 36.5), p(40.0, 45.0)], ket_stroke);
            let digit_rect =
                egui::Rect::from_center_size(p(33.0, 36.5), rect.size() * DETECTOR_DIGIT_SCALE);
            super::svg_icon::draw_glyph(
                painter,
                digit_rect,
                digit_color,
                super::svg_icon::GateGlyph::Digit0,
            );
            painter.circle_filled(p(24.0, 24.0), DETECTOR_CONTROL_RADIUS * scale, color);
        }
        GateKind::Measurement | GateKind::XDetector | GateKind::YDetector => {
            let axis_color = if grabbed {
                colors.label
            } else {
                colors.semantic_intermediate
            };
            draw_sampled_measurement_icon(painter, rect, kind, color, axis_color);
        }
        GateKind::Spacer => {
            let rect_at = |x: f32| {
                egui::Rect::from_min_max(
                    map_svg_point_in_rect(rect, SvgPoint::new(x, 21.0), viewbox),
                    map_svg_point_in_rect(rect, SvgPoint::new(x + 6.0, 27.0), viewbox),
                )
            };
            painter.rect_filled(rect_at(9.0), egui::CornerRadius::ZERO, color);
            painter.rect_filled(rect_at(21.0), egui::CornerRadius::ZERO, color);
            painter.rect_filled(rect_at(33.0), egui::CornerRadius::ZERO, color);
        }
        GateKind::Write0 | GateKind::Write1 => {
            painter.line_segment([p(6.0, 5.0), p(6.0, 43.0)], stroke);
            painter.line_segment([p(37.4516, 5.0), p(43.5, 24.0)], stroke);
            painter.line_segment([p(43.5, 24.0), p(37.4516, 43.0)], stroke);
            let (glyph, digit_color) = if kind == GateKind::Write0 {
                (super::svg_icon::GateGlyph::Digit0, colors.semantic_off)
            } else {
                (super::svg_icon::GateGlyph::Digit1, colors.semantic_on)
            };
            let digit_color = if grabbed { colors.label } else { digit_color };
            // Ket digit uses Geist Mono from the shared SVG/SDF path so the
            // slashed "0" stays unambiguous and the palette/circuit weight
            // does not depend on egui text rasterization.
            super::svg_icon::draw_glyph(painter, rect, digit_color, glyph);
        }
        GateKind::ZParityControl | GateKind::XParityControl => {
            let axis = if kind == GateKind::ZParityControl {
                "Z"
            } else {
                "X"
            };
            let center = rect.center();
            painter.text(
                center - egui::vec2(0.0, crate::tailwind::SPACING_2),
                egui::Align2::CENTER_CENTER,
                axis,
                egui::FontId::proportional(crate::tailwind::TEXT_BASE),
                colors.label,
            );
            painter.text(
                center + egui::vec2(0.0, crate::tailwind::SPACING_2),
                egui::Align2::CENTER_CENTER,
                "par",
                egui::FontId::proportional(crate::tailwind::TEXT_XS),
                colors.label,
            );
        }
        GateKind::Control => {
            painter.circle_filled(p(24.0, 24.0), CONTROL_RADIUS * scale, color);
        }
        GateKind::AntiControl => {
            let anti_control_stroke = egui::Stroke::new(ANTI_CONTROL_STROKE_WIDTH * scale, color);
            painter.circle_stroke(
                p(24.0, 24.0),
                ANTI_CONTROL_RADIUS * scale,
                anti_control_stroke,
            );
        }
        GateKind::Swap => {
            let swap_stroke = egui::Stroke::new(4.0 * scale, color);
            painter.line_segment([p(12.0, 36.0), p(36.0, 12.0)], swap_stroke);
            painter.line_segment([p(12.0, 12.0), p(36.0, 36.0)], swap_stroke);
        }
        // Everything else (H / X-as-`+` / Y / Z / √X / S / S† / T /
        // T† / Phase(Φ) / RX / RY / RZ / QFT / QFT†) was already handled above.
        _ => {}
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum ArithmeticSign {
    Plus,
    Minus,
}

const ARITHMETIC_GLYPH_SCALE: f32 = 0.02208;
const ARITHMETIC_CAP_HEIGHT: f32 = 710.0;
const ARITHMETIC_STROKE: f32 = 96.0;
const ARITHMETIC_SIGN_WIDTH: f32 = 520.0;
const ARITHMETIC_OPERAND_X: f32 = 576.0;
const ARITHMETIC_INK_WIDTH: f32 = 855.0;
const ARITHMETIC_ONE_OUTLINES: [&[[f32; 2]]; 3] = [
    &[
        [183.0, 0.0],
        [279.0, 0.0],
        [279.0, 710.0],
        [204.0, 710.0],
        [183.0, 631.0],
    ],
    &[[34.0, 522.0], [204.0, 522.0], [204.0, 606.0], [34.0, 606.0]],
    &[[97.0, 560.0], [204.0, 560.0], [204.0, 638.0], [97.0, 606.0]],
];

fn draw_increment_icon(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    draw_arithmetic_icon(painter, rect, color, ArithmeticSign::Plus);
}

fn draw_decrement_icon(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    draw_arithmetic_icon(painter, rect, color, ArithmeticSign::Minus);
}

fn draw_arithmetic_icon(
    painter: &egui::Painter,
    rect: egui::Rect,
    color: egui::Color32,
    sign: ArithmeticSign,
) {
    let origin_x = VIEWBOX * 0.5 - ARITHMETIC_INK_WIDTH * ARITHMETIC_GLYPH_SCALE * 0.5;
    let baseline = VIEWBOX * 0.5 + ARITHMETIC_CAP_HEIGHT * ARITHMETIC_GLYPH_SCALE * 0.5;
    let p = |x: f32, y: f32| {
        map_svg_point_in_rect(
            rect,
            SvgPoint::new(
                origin_x + x * ARITHMETIC_GLYPH_SCALE,
                baseline - y * ARITHMETIC_GLYPH_SCALE,
            ),
            VIEWBOX,
        )
    };
    let bar = |center: [f32; 2], half_size: [f32; 2]| {
        painter.rect_filled(
            egui::Rect::from_two_pos(
                p(center[0] - half_size[0], center[1] - half_size[1]),
                p(center[0] + half_size[0], center[1] + half_size[1]),
            ),
            egui::CornerRadius::ZERO,
            color,
        );
    };
    let center = [ARITHMETIC_SIGN_WIDTH * 0.5, ARITHMETIC_CAP_HEIGHT * 0.5];
    bar(
        center,
        [ARITHMETIC_SIGN_WIDTH * 0.5, ARITHMETIC_STROKE * 0.5],
    );
    if sign == ArithmeticSign::Plus {
        bar(
            center,
            [ARITHMETIC_STROKE * 0.5, ARITHMETIC_SIGN_WIDTH * 0.5],
        );
    }
    for outline in ARITHMETIC_ONE_OUTLINES {
        painter.add(egui::Shape::convex_polygon(
            outline
                .iter()
                .map(|&[x, y]| p(ARITHMETIC_OPERAND_X + x, y))
                .collect(),
            color,
            egui::Stroke::NONE,
        ));
    }
}

fn svg_glyph_for(kind: GateKind) -> Option<super::svg_icon::GateGlyph> {
    use super::svg_icon::GateGlyph;
    Some(match kind {
        GateKind::H => GateGlyph::H,
        GateKind::Y => GateGlyph::Y,
        GateKind::Z => GateGlyph::Z,
        GateKind::SqrtX => GateGlyph::SqrtX,
        GateKind::S => GateGlyph::S,
        GateKind::SDagger => GateGlyph::SDagger,
        GateKind::T => GateGlyph::T,
        GateKind::TDagger => GateGlyph::TDagger,
        GateKind::Phase => GateGlyph::Phase,
        GateKind::Rx => GateGlyph::Rx,
        GateKind::Ry => GateGlyph::Ry,
        GateKind::Rz => GateGlyph::Rz,
        GateKind::QftGate => GateGlyph::Qft,
        GateKind::QftDaggerGate => GateGlyph::QftDagger,
        // X ゲートの本体は qni の慣例で "+" (CNOT ターゲットと同じ)。
        GateKind::X => GateGlyph::Plus,
        _ => return None,
    })
}
