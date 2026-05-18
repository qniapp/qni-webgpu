use eframe::egui;

use crate::app::QniApp;
use crate::colors::Colors;
use crate::constants::GATE_SIZE;
use crate::gates::GateKind;
use crate::layout::LayoutMetrics;
use crate::simulation_plan::ColumnAnalysis;

use super::super::circuit::gate_slot_index_for_render;
use super::{CONNECTOR_STROKE_WIDTH, CONNECTOR_VISUAL_X_OFFSET};

pub(super) fn draw_control_connectors(
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
        let mut controls = Vec::new();
        let mut targets = Vec::new();
        for gate in column.gates() {
            if gate.kind == GateKind::Swap {
                continue;
            }
            let center =
                circuit_origin + gate.pos.to_vec2() + egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0);
            if gate.kind == GateKind::Control || gate.kind == GateKind::AntiControl {
                controls.push(center);
            } else {
                targets.push(center);
            }
        }
        // Connector is a *control-only* affordance: it tells the reader "this
        // column is a multi-qubit controlled operation". Columns with no
        // controls (e.g. four parallel Hs, parallel Blochs, parallel writes)
        // are independent single-qubit gates and must NOT get a line —
        // matching qni's `circuit-step-element.ts:526` early-return when both
        // control lists are empty.
        if controls.is_empty() {
            continue;
        }
        // A lone control with no controllable target is a disabled no-op in qni
        // (`:513-524`); we likewise skip the line for it.
        if targets.is_empty() && controls.len() < 2 {
            continue;
        }
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for point in controls.iter().chain(targets.iter()) {
            min_y = min_y.min(point.y);
            max_y = max_y.max(point.y);
        }
        // Anchor the line at the *slot center*, not the mean of the gate
        // centers — during a drag the moving gate may sit a few pixels off the
        // slot midpoint even while it's inside `SNAP_DISTANCE`, and averaging
        // would pull the line off the column the snap is actually going to
        // commit to.
        let x = circuit_origin.x + metrics.slot_centers[column.slot] + CONNECTOR_VISUAL_X_OFFSET;
        let stroke = egui::Stroke::new(CONNECTOR_STROKE_WIDTH, colors.box_fill);
        painter.line_segment([egui::pos2(x, min_y), egui::pos2(x, max_y)], stroke);
    }
}
