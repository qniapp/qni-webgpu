use eframe::egui;
use std::cmp::Ordering;

use crate::app::QniApp;
use crate::colors::Colors;
use crate::constants::GATE_SIZE;
use crate::gates::GateKind;
use crate::layout::LayoutMetrics;
use crate::simulation_plan::ColumnAnalysis;

use super::super::circuit::gate_slot_index_for_render;

pub(super) fn draw_swap_connectors(
    app: &QniApp,
    painter: &egui::Painter,
    metrics: &LayoutMetrics,
    colors: &Colors,
    circuit_origin: egui::Pos2,
    dragging_gate_id: Option<u32>,
) {
    let analysis = ColumnAnalysis::from_gates(&app.placed_gates, |gate| {
        gate_slot_index_for_render(gate, metrics, dragging_gate_id)
    });

    for column in analysis.columns() {
        let gates = column
            .gates()
            .iter()
            .copied()
            .filter(|gate| gate.kind == GateKind::Swap)
            .collect::<Vec<_>>();
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
        let x = circuit_origin.x + metrics.slot_centers[column.slot];
        let swap_stroke = egui::Stroke::new(GATE_SIZE / 12.0, colors.box_fill);
        painter.line_segment([egui::pos2(x, top_y), egui::pos2(x, bottom_y)], swap_stroke);
    }
}
