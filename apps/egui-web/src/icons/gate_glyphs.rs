//! Gate icon glyphs translated from qni SVG primitives.

use eframe::egui;

use crate::colors::Colors;
use crate::gates::GateKind;

use super::bloch::draw_bloch_sphere;
use super::svg::{map_svg_point_in_rect, SvgPoint};
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
    // H / Y / Z / √X / S / S† / T / T† / P / RX / RY / RZ / QFT /
    // QFT† / X (本体は "+" として描く) は
    // `assets/icons/{h,y,z,sqrtx,s,sdagger,t,tdagger,p,rx,ry,rz,qft,qftdagger,plus}.svg`
    // を 1 ソースに、ビルド時に 256 px PNG から作った SDF テクスチャで
    // 描画する。パレットと回路のサブピクセル位置差で見かけの太さが揺れず、
    // 拡大時も輪郭をシェーダで再構成できるようにする。
    if let Some(glyph) = svg_glyph_for(kind) {
        super::svg_icon::draw_glyph(painter, rect, color, glyph);
        return true;
    }

    let viewbox = VIEWBOX;
    let scale = rect.width() / viewbox;
    let stroke = egui::Stroke::new(2.0 * scale, color);
    let p = |x: f32, y: f32| map_svg_point_in_rect(rect, SvgPoint::new(x, y), viewbox);

    match kind {
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
            // Ket digit uses Geist Mono so the slashed "0" is
            // unambiguously a zero rather than an "O". Size stays
            // below the large single-letter gates so the digit sits
            // cleanly between the bracket strokes.
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                digit,
                egui::FontId::new(rect.width() * 0.56, egui::FontFamily::Monospace),
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
        GateKind::Swap => {
            let swap_stroke = egui::Stroke::new(4.0 * scale, color);
            painter.line_segment([p(12.0, 36.0), p(36.0, 12.0)], swap_stroke);
            painter.line_segment([p(12.0, 12.0), p(36.0, 36.0)], swap_stroke);
            true
        }
        // Everything else (H / X-as-`+` / Y / Z / √X / S / S† / T /
        // T† / P / RX / RY / RZ / QFT / QFT†) was already handled above.
        // Anything that reaches here without matching is a non-typographic gate
        // the body code knows how to fall back on with `kind.label()`.
        _ => false,
    }
}

/// SVG 起点でテクスチャ描画するゲートに対する `GateGlyph` への対応。
/// ここに登録された GateKind は `painter.text` を介さず
/// `assets/icons/*.svg` から作ったテクスチャで描く。
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
        GateKind::Phase => GateGlyph::P,
        GateKind::Rx => GateGlyph::Rx,
        GateKind::Ry => GateGlyph::Ry,
        GateKind::Rz => GateGlyph::Rz,
        GateKind::QftGate => GateGlyph::Qft,
        GateKind::QftDaggerGate => GateGlyph::QftDagger,
        // X ゲートの本体は qni の慣例で "+" (CNOT ターゲットと同じ)。
        // 文字 "X" 用の SVG はパレットや将来の単独使用向けに別管理だが、
        // ここでは Plus を返す。
        GateKind::X => GateGlyph::Plus,
        _ => return None,
    })
}
