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
    // Typographic gates (H / X-as-`+` / Y / Z / √X / S / S† / T / T† /
    // P / RX / RY / RZ / QFT / QFT†) are rendered as Geist Bold text
    // via `painter.text` instead of hand-extracted SVG strokes. The
    // dagger gates render the base letter centred and the † as a
    // smaller mark in the top-right corner so the central glyph stays
    // legible at 32 px.
    if let Some(label) = base_label_for(kind) {
        draw_text_label(painter, rect, kind, label, color);
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
        GateKind::Swap => {
            let swap_stroke = egui::Stroke::new(4.0 * scale, color);
            painter.line_segment([p(12.0, 36.0), p(36.0, 12.0)], swap_stroke);
            painter.line_segment([p(12.0, 12.0), p(36.0, 36.0)], swap_stroke);
            true
        }
        // Everything else (H / X-as-`+` / Y / Z / √X / S / S† / T /
        // T† / P / RX / RY / RZ / QFT / QFT†) was already handled by
        // `base_label_for` at the top via Geist Bold text. Anything that
        // reaches here without matching is a non-typographic gate the
        // body code knows how to fall back on with `kind.label()`.
        _ => false,
    }
}

/// Map a `GateKind` to the base text label rendered at the centre of
/// its body. Daggered variants share the same base letter as their
/// non-daggered sibling; the dagger mark is added separately by
/// `draw_text_label`.
fn base_label_for(kind: GateKind) -> Option<&'static str> {
    Some(match kind {
        GateKind::H => "H",
        // qni renders the X gate body as a filled circle with a `+`
        // glyph centred inside (CNOT-target convention). Keep that.
        GateKind::X => "+",
        GateKind::Y => "Y",
        GateKind::Z => "Z",
        GateKind::SqrtX => "√X",
        GateKind::S | GateKind::SDagger => "S",
        GateKind::T | GateKind::TDagger => "T",
        GateKind::Phase => "P",
        // R-axis rotations: at GATE_SIZE 32 px a subscript x / y / z
        // is sub-5 px and unreadable. `icons.rs`'s old hand-drawn
        // glyphs already rendered the axis letter at the same height
        // as R, so typeset as `RX` / `RY` / `RZ` to match.
        GateKind::Rx => "RX",
        GateKind::Ry => "RY",
        GateKind::Rz => "RZ",
        GateKind::QftGate | GateKind::QftDaggerGate => "QFT",
        _ => return None,
    })
}

fn is_dagger_variant(kind: GateKind) -> bool {
    matches!(
        kind,
        GateKind::SDagger | GateKind::TDagger | GateKind::QftDaggerGate
    )
}

/// Pixel font size for the base label, derived from the gate-body
/// width. Ratios picked on the gate-label mockup (Geist Bold 700);
/// stays consistent across the 32 px palette and any future scale.
fn base_label_font_px(label: &str, body_px: f32) -> f32 {
    let ratio = if label == "√X" {
        // √X needs extra headroom — the radical extends above the X
        // cap-height and clips the gate body if scaled like 2 plain chars.
        0.41
    } else if label.chars().count() == 1 {
        0.47
    } else {
        // RX / RY / RZ / QFT — 2-3 chars need a smaller size to fit
        // two-glyph width inside the body.
        0.31
    };
    body_px * ratio
}

fn draw_text_label(
    painter: &egui::Painter,
    rect: egui::Rect,
    kind: GateKind,
    label: &str,
    color: egui::Color32,
) {
    let body_px = rect.width();
    let font = egui::FontId::new(
        base_label_font_px(label, body_px),
        crate::app::GATE_LABEL_FAMILY.clone(),
    );
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        font,
        color,
    );
    if is_dagger_variant(kind) {
        // Dagger sits at the body's top-right so the base letter stays
        // dead-centre. Size ≈ 0.28× body; insets ≈ 0.13× / 0.18× match
        // the gate-label mockup's `top:3 right:4` overlay at 32 px.
        let dag_font = egui::FontId::new(body_px * 0.28, crate::app::GATE_LABEL_FAMILY.clone());
        let inset_x = body_px * 0.13;
        let inset_y = body_px * 0.18;
        painter.text(
            egui::pos2(rect.right() - inset_x, rect.top() + inset_y),
            egui::Align2::CENTER_CENTER,
            "†",
            dag_font,
            color,
        );
    }
}
