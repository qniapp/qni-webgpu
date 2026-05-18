//! Gate body fill, special bodies, and fallback label drawing.

use eframe::egui;

use crate::colors::Colors;
use crate::constants::GATE_SIZE;
use crate::gates::GateKind;

use super::gate_glyphs::draw_gate_icon;

const CHANCE_PREVIEW_BAR_WIDTHS: [f32; 4] = [0.30, 0.75, 0.55, 0.20];

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
    } else if kind == GateKind::ChanceDisplay {
        draw_chance_preview_body(painter, gate_rect, colors);
        return;
    } else if kind == GateKind::Phase {
        // qni renders the parametric Phase as a circular body (the
        // SDF-backed `P` glyph centred inside) so the angle label has
        // somewhere clean to sit above / below the gate without colliding with
        // the gate body's square corners.
        let radius = gate_rect.width().min(gate_rect.height()) / 2.0;
        painter.circle_filled(gate_rect.center(), radius, fill);
    } else if matches!(kind, GateKind::QftGate | GateKind::QftDaggerGate) {
        // QFT family — same green body as the other unitary gates. The
        // letter rendering is delegated to `draw_gate_icon` along with
        // every other SVG/SDF-backed typographic gate. For multi-qubit
        // spans the icon is anchored to a GATE_SIZE square at the vertical
        // centre so the lettering stays put even when the body is taller.
        painter.rect_filled(gate_rect, egui::CornerRadius::same(6), fill);
        let cx = gate_rect.center().x;
        let cy = gate_rect.center().y;
        let half = GATE_SIZE * 0.5;
        let icon_rect = egui::Rect::from_min_max(
            egui::pos2(cx - half, cy - half),
            egui::pos2(cx + half, cy + half),
        );
        draw_gate_icon(painter, icon_rect, kind, colors.label, colors);
        return;
    } else if kind == GateKind::BlochDisplay {
        // qni renders the bloch display as a stand-alone sphere — bg-green-50
        // background with a gray-400 border (`packages/elements/css/bloch_display.css`).
        let radius = gate_rect.width().min(gate_rect.height()) * 0.5 - 1.0;
        painter.circle_filled(gate_rect.center(), radius, colors.bloch_sphere_bg);
        painter.circle_stroke(
            gate_rect.center(),
            radius,
            egui::Stroke::new(1.5, colors.bloch_sphere_lines),
        );
    } else if kind != GateKind::Control
        && kind != GateKind::AntiControl
        && kind != GateKind::Swap
        && kind != GateKind::Measurement
        && kind != GateKind::Spacer
        && kind != GateKind::Write0
        && kind != GateKind::Write1
    {
        painter.rect_filled(gate_rect, egui::CornerRadius::same(6), fill);
    }
    let icon_color =
        if kind == GateKind::Control || kind == GateKind::AntiControl || kind == GateKind::Swap {
            fill
        } else if kind == GateKind::BlochDisplay {
            colors.bloch_sphere_lines
        } else if kind == GateKind::Measurement {
            // qni `measurement_gate.css`: icon color is semantic-color-intermediate (purple).
            colors.semantic_intermediate
        } else if kind == GateKind::Spacer {
            // qni `spacer_gate.css`: text-neutral-900 (#171717).
            colors.spacer_dots
        } else if kind == GateKind::Write0 || kind == GateKind::Write1 {
            // qni `write_gate.css`: ::part(icon) (the brackets) is
            // semantic-fill-color-disabled (zinc-500). Only the digit itself
            // is red/blue — handled inside draw_gate_icon.
            colors.semantic_disabled
        } else {
            colors.label
        };
    if !draw_gate_icon(painter, gate_rect, kind, icon_color, colors) {
        painter.text(
            gate_rect.center(),
            egui::Align2::CENTER_CENTER,
            kind.label(),
            egui::FontId::proportional(18.0),
            colors.label,
        );
    }
}

fn draw_chance_preview_body(painter: &egui::Painter, rect: egui::Rect, colors: &Colors) {
    // Static mini-preview only. Live probabilities are drawn later by the
    // GPU Chance render callback, directly from `chance_probability_output`.
    painter.rect_filled(rect, egui::CornerRadius::ZERO, colors.surface);

    // `docs/chance-display-icon-options.html` の案 1。4 行のガウス風分布にして、
    // 32 px パレット上でも「複数結果のヒストグラム」と分かるようにする。
    let row_h = rect.height() / CHANCE_PREVIEW_BAR_WIDTHS.len() as f32;
    for (row, &width_ratio) in CHANCE_PREVIEW_BAR_WIDTHS.iter().enumerate() {
        let bar_y = rect.top() + row_h * row as f32;
        let bar = egui::Rect::from_min_size(
            egui::pos2(rect.left(), bar_y),
            egui::vec2(rect.width() * width_ratio, row_h),
        );
        painter.rect_filled(bar, egui::CornerRadius::ZERO, colors.state_fill);
    }
    for row in 1..CHANCE_PREVIEW_BAR_WIDTHS.len() {
        let y = rect.top() + row_h * row as f32;
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            egui::Stroke::new(1.0, colors.line),
        );
    }
    // 回路側の chance display 外周 (`circuit_gates.rs:180` で `colors.line` =
    // ui-2) と階調を合わせる。Quirk の `lightgray` (#D3D3D3) と同じ薄灰で
    // 「データ領域の輪郭」だけを示し、ゲート本体の強調はバー (blue-200)
    // に任せる。
    painter.rect_stroke(
        rect,
        egui::CornerRadius::ZERO,
        egui::Stroke::new(1.0, colors.line),
        egui::StrokeKind::Inside,
    );
}
