use eframe::egui;
use std::collections::HashMap;

use crate::app::QniApp;
use crate::colors::Colors;
use crate::constants::GATE_SIZE;
use crate::gates::GateKind;
use crate::layout::LayoutMetrics;

use super::super::circuit::gate_slot_index_for_render;

pub(super) fn draw_phase_connectors_and_labels(
    app: &QniApp,
    painter: &egui::Painter,
    metrics: &LayoutMetrics,
    colors: &Colors,
    circuit_origin: egui::Pos2,
    dragging_gate_id: Option<u32>,
) {
    draw_phase_phase_connectors(
        app,
        painter,
        metrics,
        colors,
        circuit_origin,
        dragging_gate_id,
    );
    draw_phase_angle_labels(
        app,
        painter,
        metrics,
        colors,
        circuit_origin,
        dragging_gate_id,
    );
}

fn draw_phase_phase_connectors(
    app: &QniApp,
    painter: &egui::Painter,
    metrics: &LayoutMetrics,
    colors: &Colors,
    circuit_origin: egui::Pos2,
    dragging_gate_id: Option<u32>,
) {
    // Phase-Phase connector. qni's
    // `circuit-step-element.ts::updatePhasePhaseConnections` (:566-602)
    // draws a connector between same-angle Phase gates in the same column.
    // Semantically it's a *visual* pairing only — qni's simulator still runs
    // each Phase independently (`simulator.ts::cu` :413-417 loops over
    // targets and applies the same 2x2 to each in turn), so we mirror just the
    // line rendering. Phases with no angle (qni's empty placeholder) are
    // skipped per :573.
    let mut phase_groups: HashMap<usize, HashMap<String, Vec<egui::Pos2>>> = HashMap::new();
    for gate in &app.placed_gates {
        if gate.kind != GateKind::Phase {
            continue;
        }
        let angle = gate.angle.as_deref().unwrap_or("");
        if angle.is_empty() {
            continue;
        }
        let Some(slot_index) = gate_slot_index_for_render(gate, metrics, dragging_gate_id) else {
            continue;
        };
        let center =
            circuit_origin + gate.pos.to_vec2() + egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0);
        phase_groups
            .entry(slot_index)
            .or_default()
            .entry(angle.to_string())
            .or_default()
            .push(center);
    }
    for (slot_index, angle_buckets) in &phase_groups {
        for points in angle_buckets.values() {
            if points.len() < 2 {
                continue;
            }
            let mut min_y = f32::INFINITY;
            let mut max_y = f32::NEG_INFINITY;
            for point in points {
                min_y = min_y.min(point.y);
                max_y = max_y.max(point.y);
            }
            // Slot-center anchored — same rationale as the control / swap
            // connectors.
            let x = circuit_origin.x + metrics.slot_centers[*slot_index];
            let stroke = egui::Stroke::new(GATE_SIZE / 12.0, colors.box_fill);
            painter.line_segment([egui::pos2(x, min_y), egui::pos2(x, max_y)], stroke);
        }
    }
}

fn draw_phase_angle_labels(
    app: &QniApp,
    painter: &egui::Painter,
    metrics: &LayoutMetrics,
    colors: &Colors,
    circuit_origin: egui::Pos2,
    dragging_gate_id: Option<u32>,
) {
    // Angle labels for Phase gates. qni puts the angle text just outside the
    // circular gate body (above for the topmost / standalone gate in a
    // same-angle pair, below for the bottommost) so the label never overlaps
    // the vertical connector that ties same-angle Phase gates together. We
    // replicate the same dodge logic here.
    for gate in &app.placed_gates {
        if gate.kind != GateKind::Phase {
            continue;
        }
        let Some(angle) = gate.angle.as_deref() else {
            continue;
        };
        if angle.is_empty() {
            continue;
        }
        let Some(slot_index) = gate_slot_index_for_render(gate, metrics, dragging_gate_id) else {
            continue;
        };
        let center =
            circuit_origin + gate.pos.to_vec2() + egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0);

        let (peers_above, peers_below) = phase_peers_by_side(
            app,
            gate.id,
            gate.wire,
            angle,
            slot_index,
            metrics,
            dragging_gate_id,
        );

        // Above for the topmost / standalone gate; below for the bottommost. A
        // middle gate in a 3+ chain falls back to above and is left to overlap
        // the connector (qni does the same).
        //   standalone (no peers)         → above
        //   topmost (peer below only)     → above
        //   bottommost (peer above only)  → below
        //   middle (peers above & below)  → above (fallback)
        let label_above = peers_below || !peers_above;
        let (label_y, align) = if label_above {
            (
                center.y - GATE_SIZE / 2.0 - 2.0,
                egui::Align2::CENTER_BOTTOM,
            )
        } else {
            (center.y + GATE_SIZE / 2.0 + 2.0, egui::Align2::CENTER_TOP)
        };
        painter.text(
            egui::pos2(center.x, label_y),
            align,
            angle,
            // text-xs (12 px) — Tailwind. Matches the popup body font so labels
            // feel like they belong to the same typographic system.
            egui::FontId::monospace(12.0),
            colors.text,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn phase_peers_by_side(
    app: &QniApp,
    gate_id: u32,
    wire: usize,
    angle: &str,
    slot_index: usize,
    metrics: &LayoutMetrics,
    dragging_gate_id: Option<u32>,
) -> (bool, bool) {
    let mut peers_above = false;
    let mut peers_below = false;
    for other in &app.placed_gates {
        if other.id == gate_id || other.kind != GateKind::Phase {
            continue;
        }
        if other.angle.as_deref() != Some(angle) {
            continue;
        }
        let Some(other_slot) = gate_slot_index_for_render(other, metrics, dragging_gate_id) else {
            continue;
        };
        if other_slot != slot_index {
            continue;
        }
        if other.wire < wire {
            peers_above = true;
        } else if other.wire > wire {
            peers_below = true;
        }
    }
    (peers_above, peers_below)
}
