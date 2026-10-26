use eframe::egui;

use crate::app::{GateId, QniApp};
use crate::colors::Colors;
use crate::gates::GateKind;
use crate::layout::{CircuitProjection, LayoutMetrics};
use crate::simulation_plan::ColumnAnalysis;

use super::super::circuit::gate_slot_index_for_render;
use super::{along_wires, draw_wire_connector, screen_time};

// The anti-control icon is a hollow 40px-gate glyph; split the vertical
// connector through its hollow center while leaving the line visible between
// the anti-control and its target.
const ANTI_CONTROL_CONNECTOR_CLEAR_RADIUS: f32 = 6.0;

pub(super) fn draw_control_connectors(
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
        let mut controls = Vec::new();
        let mut targets = Vec::new();
        let mut anti_control_gaps = Vec::new();
        for gate in column.gates() {
            let center = crate::render::circuit::gate_square_center(
                projection,
                crate::render::circuit::gate_draw_pos(gate, app.dragging),
            );
            if matches!(
                gate.kind,
                GateKind::Control
                    | GateKind::AntiControl
                    | GateKind::ZDetectControlReset
                    | GateKind::ZParityControl
                    | GateKind::XParityControl
            ) {
                controls.push(center);
                if gate.kind == GateKind::AntiControl {
                    anti_control_gaps.push(along_wires(projection, center));
                }
            } else {
                targets.push(center);
            }
        }
        if controls.is_empty() {
            continue;
        }
        if targets.is_empty() && controls.len() < 2 {
            continue;
        }
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for &point in controls.iter().chain(targets.iter()) {
            min_y = min_y.min(along_wires(projection, point));
            max_y = max_y.max(along_wires(projection, point));
        }
        // Anchor the line at the shared slot center. Dragged gates only join
        // this render column after their preview center is on the real slot;
        // insert previews stay out so the connector never drifts away from a
        // transparent Control / AntiControl body.
        let time = screen_time(projection, metrics.slot_centers[column.slot]);
        for (start_y, end_y) in connector_segments(min_y, max_y, &anti_control_gaps) {
            draw_wire_connector(painter, projection, time, start_y, end_y, colors.box_fill);
        }
    }
}

fn connector_segments(min_y: f32, max_y: f32, anti_control_centers: &[f32]) -> Vec<(f32, f32)> {
    let mut gaps = anti_control_centers
        .iter()
        .map(|center| {
            (
                (center - ANTI_CONTROL_CONNECTOR_CLEAR_RADIUS).max(min_y),
                (center + ANTI_CONTROL_CONNECTOR_CLEAR_RADIUS).min(max_y),
            )
        })
        .collect::<Vec<_>>();
    gaps.sort_by(|left, right| left.0.total_cmp(&right.0));

    let mut segments = Vec::new();
    let mut cursor = min_y;
    for (gap_start, gap_end) in gaps {
        if gap_end <= cursor {
            continue;
        }
        if gap_start > cursor {
            segments.push((cursor, gap_start));
        }
        cursor = cursor.max(gap_end);
    }
    if cursor < max_y {
        segments.push((cursor, max_y));
    }
    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn z_detect_control_reset_connector_snapshot() {
        let ctx = egui::Context::default();
        let mut app = QniApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()));
        app.load_circuit_json_into_editor(r#"{"cols":[["ZDetectControlReset","X"]]}"#, &ctx);
        let metrics = crate::layout::layout_metrics(800.0, 2, 6);
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            draw_control_connectors(
                &app,
                ui.painter(),
                &metrics,
                &app.colors(),
                CircuitProjection::horizontal(egui::Pos2::ZERO),
                None,
            );
        });
        insta::assert_debug_snapshot!(output.shapes);
    }
}
