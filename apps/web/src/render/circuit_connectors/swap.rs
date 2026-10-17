use eframe::egui;
use std::cmp::Ordering;

use crate::app::{GateId, QniApp};
use crate::colors::Colors;
use crate::gates::GateKind;
use crate::layout::{CircuitProjection, LayoutMetrics};
use crate::simulation_plan::ColumnAnalysis;

use super::super::circuit::gate_slot_index_for_render;
use super::{along_time, along_wires, draw_wire_connector};

pub(super) fn draw_swap_connectors(
    app: &QniApp,
    painter: &egui::Painter,
    metrics: &LayoutMetrics,
    colors: &Colors,
    projection: CircuitProjection,
    dragging_gate_id: Option<GateId>,
) {
    let analysis = ColumnAnalysis::from_gates(&app.placed_gates, |gate| {
        gate_slot_index_for_render(
            gate,
            metrics,
            app.dragging
                .filter(|drag| Some(drag.id) == dragging_gate_id),
        )
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
        let centers = gates
            .iter()
            .map(|gate| {
                crate::render::circuit::gate_square_center(
                    projection,
                    crate::render::circuit::gate_draw_pos(gate, app.dragging),
                )
            })
            .collect::<Vec<_>>();
        let mut ys = centers
            .iter()
            .map(|&center| along_wires(projection, center))
            .collect::<Vec<_>>();
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
        let top_y = *ys.first().unwrap();
        let bottom_y = *ys.last().unwrap();
        // Swap connectors visually join the swap symbols themselves. Use the
        // actual rendered gate centres instead of the slot centre so the line
        // stays centred in the transparent Swap body during hover / drag.
        let time = centers
            .iter()
            .map(|&center| along_time(projection, center))
            .sum::<f32>()
            / gates.len() as f32;
        draw_wire_connector(painter, projection, time, top_y, bottom_y, colors.box_fill);
    }
}
