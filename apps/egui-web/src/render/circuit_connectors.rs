//! Circuit connector and phase-label drawing.

use eframe::egui;
use std::cmp::Ordering;
use std::collections::HashMap;

use crate::app::{PlacedGate, QniApp};
use crate::colors::Colors;
use crate::constants::GATE_SIZE;
use crate::gates::GateKind;
use crate::layout::LayoutMetrics;

use super::circuit::gate_slot_index_for_render;

impl QniApp {
    pub(super) fn draw_circuit_connectors(
        &self,
        painter: &egui::Painter,
        metrics: &LayoutMetrics,
        colors: &Colors,
        circuit_origin: egui::Pos2,
        dragging_gate_id: Option<u32>,
    ) {
        // Connector lines (CNOT / CZ / Swap / Phase-Phase) are computed
        // every frame, including mid-drag, so a gate being moved into
        // a CNOT pair (or out of one) shows the line snapping live
        // instead of waiting for the drop. The work is cheap — one
        // pass over `placed_gates` per group — and well under the
        // dispatch budget at our 16-qubit cap.
        {
            let mut control_groups: HashMap<usize, (Vec<egui::Pos2>, Vec<egui::Pos2>)> =
                HashMap::new();
            for gate in &self.placed_gates {
                if gate.kind == GateKind::Swap {
                    continue;
                }
                let is_control =
                    gate.kind == GateKind::Control || gate.kind == GateKind::AntiControl;
                let Some(slot_index) = gate_slot_index_for_render(gate, metrics, dragging_gate_id)
                else {
                    continue;
                };
                let center = circuit_origin
                    + gate.pos.to_vec2()
                    + egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0);
                let entry = control_groups
                    .entry(slot_index)
                    .or_insert((Vec::new(), Vec::new()));
                if is_control {
                    entry.0.push(center);
                } else {
                    entry.1.push(center);
                }
            }

            for (slot_index, (controls, targets)) in &control_groups {
                // Connector is a *control-only* affordance: it tells
                // the reader "this column is a multi-qubit controlled
                // operation". Columns with no controls (e.g. four
                // parallel Hs, parallel Blochs, parallel writes) are
                // independent single-qubit gates and must NOT get a
                // line — matching qni's `circuit-step-element.ts:526`
                // early-return when both control lists are empty.
                if controls.is_empty() {
                    continue;
                }
                // A lone control with no controllable target is a
                // disabled no-op in qni (`:513-524`); we likewise skip
                // the line for it.
                if targets.is_empty() && controls.len() < 2 {
                    continue;
                }
                let mut min_y = f32::INFINITY;
                let mut max_y = f32::NEG_INFINITY;
                for point in controls.iter().chain(targets.iter()) {
                    min_y = min_y.min(point.y);
                    max_y = max_y.max(point.y);
                }
                // Anchor the line at the *slot center*, not the mean
                // of the gate centers — during a drag the moving gate
                // may sit a few pixels off the slot midpoint even
                // while it's inside `SNAP_DISTANCE`, and averaging
                // would pull the line off the column the snap is
                // actually going to commit to.
                let x = circuit_origin.x + metrics.slot_centers[*slot_index];
                let stroke = egui::Stroke::new(GATE_SIZE / 12.0, colors.box_fill);
                painter.line_segment([egui::pos2(x, min_y), egui::pos2(x, max_y)], stroke);
            }

            let mut swap_groups: HashMap<usize, Vec<&PlacedGate>> = HashMap::new();
            for gate in &self.placed_gates {
                if gate.kind != GateKind::Swap {
                    continue;
                }
                if let Some(slot_index) =
                    gate_slot_index_for_render(gate, metrics, dragging_gate_id)
                {
                    swap_groups.entry(slot_index).or_default().push(gate);
                }
            }

            for (slot_index, gates) in &swap_groups {
                if gates.len() < 2 {
                    continue;
                }
                let mut ys = gates
                    .iter()
                    .map(|gate| circuit_origin.y + gate.pos.y + GATE_SIZE / 2.0)
                    .collect::<Vec<_>>();
                ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
                let top_y = *ys.first().unwrap();
                let bottom_y = *ys.last().unwrap();
                // Slot-center anchored — same rationale as the control
                // connector above.
                let x = circuit_origin.x + metrics.slot_centers[*slot_index];
                let swap_stroke = egui::Stroke::new(GATE_SIZE / 12.0, colors.box_fill);
                painter.line_segment([egui::pos2(x, top_y), egui::pos2(x, bottom_y)], swap_stroke);
            }

            // Phase-Phase connector. qni's
            // `circuit-step-element.ts::updatePhasePhaseConnections`
            // (:566-602) draws a connector between same-angle Phase
            // gates in the same column. Semantically it's a *visual*
            // pairing only — qni's simulator still runs each Phase
            // independently (`simulator.ts::cu` :413-417 loops over
            // targets and applies the same 2x2 to each in turn), so we
            // mirror just the line rendering. Phases with no angle
            // (qni's empty placeholder) are skipped per :573.
            let mut phase_groups: HashMap<usize, HashMap<String, Vec<egui::Pos2>>> = HashMap::new();
            for gate in &self.placed_gates {
                if gate.kind != GateKind::Phase {
                    continue;
                }
                let angle = gate.angle.as_deref().unwrap_or("");
                if angle.is_empty() {
                    continue;
                }
                let Some(slot_index) = gate_slot_index_for_render(gate, metrics, dragging_gate_id)
                else {
                    continue;
                };
                let center = circuit_origin
                    + gate.pos.to_vec2()
                    + egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0);
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
                    // Slot-center anchored — same rationale as the
                    // control / swap connectors above.
                    let x = circuit_origin.x + metrics.slot_centers[*slot_index];
                    let stroke = egui::Stroke::new(GATE_SIZE / 12.0, colors.box_fill);
                    painter.line_segment([egui::pos2(x, min_y), egui::pos2(x, max_y)], stroke);
                }
            }

            // Angle labels for Phase gates. qni puts the angle text just
            // outside the circular gate body (above for the topmost /
            // standalone gate in a same-angle pair, below for the
            // bottommost) so the label never overlaps the vertical
            // connector that ties same-angle Phase gates together. We
            // replicate the same dodge logic here.
            for gate in &self.placed_gates {
                if gate.kind != GateKind::Phase {
                    continue;
                }
                let Some(angle) = gate.angle.as_deref() else {
                    continue;
                };
                if angle.is_empty() {
                    continue;
                }
                let Some(slot_index) = gate_slot_index_for_render(gate, metrics, dragging_gate_id)
                else {
                    continue;
                };
                let center = circuit_origin
                    + gate.pos.to_vec2()
                    + egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0);

                // Peers in the same column with the same angle.
                let mut peers_above = false;
                let mut peers_below = false;
                for other in &self.placed_gates {
                    if other.id == gate.id || other.kind != GateKind::Phase {
                        continue;
                    }
                    if other.angle.as_deref() != Some(angle) {
                        continue;
                    }
                    let Some(other_slot) =
                        gate_slot_index_for_render(other, metrics, dragging_gate_id)
                    else {
                        continue;
                    };
                    if other_slot != slot_index {
                        continue;
                    }
                    if other.wire < gate.wire {
                        peers_above = true;
                    } else if other.wire > gate.wire {
                        peers_below = true;
                    }
                }

                // Above for the topmost / standalone gate; below for
                // the bottommost. A middle gate in a 3+ chain falls
                // back to above and is left to overlap the connector
                // (qni does the same).
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
                    // text-xs (12 px) — Tailwind. Matches the popup body
                    // font so labels feel like they belong to the same
                    // typographic system.
                    egui::FontId::monospace(12.0),
                    colors.text,
                );
            }
        }
    }
}
