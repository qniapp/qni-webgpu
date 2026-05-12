use eframe::egui;
use std::cmp::Ordering;
use std::collections::HashMap;

use crate::app::{PlacedGate, QniApp};
use crate::colors::Colors;
use crate::constants::GATE_SIZE;
use crate::gates::GateKind;
use crate::layout::LayoutMetrics;

use super::super::circuit::gate_slot_index_for_render;

pub(super) fn draw_swap_connectors(
    app: &QniApp,
    painter: &egui::Painter,
    metrics: &LayoutMetrics,
    colors: &Colors,
    circuit_origin: egui::Pos2,
    dragging_gate_id: Option<u32>,
) {
    let mut swap_groups: HashMap<usize, Vec<&PlacedGate>> = HashMap::new();
    for gate in &app.placed_gates {
        if gate.kind != GateKind::Swap {
            continue;
        }
        if let Some(slot_index) = gate_slot_index_for_render(gate, metrics, dragging_gate_id) {
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
        // Slot-center anchored — same rationale as the control connector.
        let x = circuit_origin.x + metrics.slot_centers[*slot_index];
        let swap_stroke = egui::Stroke::new(GATE_SIZE / 12.0, colors.box_fill);
        painter.line_segment([egui::pos2(x, top_y), egui::pos2(x, bottom_y)], swap_stroke);
    }
}
