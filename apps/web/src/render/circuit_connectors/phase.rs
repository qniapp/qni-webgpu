use eframe::egui;
use std::collections::HashMap;

use crate::app::{GateId, PlacedGate, QniApp, WireIndex};
use crate::colors::Colors;
use crate::constants::GATE_SIZE;
use crate::gates::{GateKind, ParametricAngle};
use crate::layout::LayoutMetrics;
use crate::simulation_plan::{AnalyzedColumn, ColumnAnalysis, PhaseGroupKey};

use super::super::circuit::gate_slot_index_for_render;
use super::draw_vertical_connector;

pub(in crate::render) const ANGLE_LABEL_FONT_SIZE: f32 = 12.0; // text-xs = 12px.
pub(in crate::render) const ANGLE_LABEL_ROW_HEIGHT: f32 = 16.0; // text-xs line-height = 16px.
pub(in crate::render) const ANGLE_UNDERLINE_BOTTOM_INSET: f32 = 2.5; // Prototype ::after bottom: 1px + 0.5px optical lift.
                                                                     // The 16px label sits centered in a 17px row; lift the stroke center slightly while keeping it below text.
                                                                     // spacing-0 = 0px. Anchor top labels at the target gate's top edge; this is
                                                                     // the midpoint between the previous too-low +2px inset and too-high -2px gap.
const ANGLE_LABEL_TOP_OFFSET: f32 = 0.0;
const ANGLE_LABEL_BOTTOM_GAP: f32 = 2.0;
// qni styles measurement variable / condition labels with `font-mono
// tracking-tighter text-xs` (`.operation-flaggable` / `.operation-ifable`):
// tracking-tighter = -0.05em = -0.6px at 12px.
const FLAG_LABEL_TRACKING: f32 = -0.6;
// qni joins `if` and the flag name with U+2009. The monospace font advances
// a thin space by a full cell, so paint the two runs apart instead: a 12px
// thin space (1/5 em = 2.4px) minus the -0.6px tracking.
const FLAG_LABEL_IF_GAP: f32 = 1.8;
const ANGLE_LABEL_OUTLINE_OFFSETS: [(f32, f32); 8] = [
    (1.0, 1.0),
    (-1.0, -1.0),
    (-1.0, 1.0),
    (1.0, -1.0),
    (0.0, 1.0),
    (0.0, -1.0),
    (-1.0, 0.0),
    (1.0, 0.0),
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ConnectionSides {
    top: bool,
    bottom: bool,
}

impl ConnectionSides {
    fn include_wire(&mut self, gate_wire: WireIndex, other_wire: WireIndex) {
        if other_wire < gate_wire {
            self.top = true;
        } else if other_wire > gate_wire {
            self.bottom = true;
        }
    }

    fn include(&mut self, other: Self) {
        self.top |= other.top;
        self.bottom |= other.bottom;
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::render) struct AngleLabelInfo {
    pub(in crate::render) gate_id: GateId,
    pub(in crate::render) text: String,
    pub(in crate::render) pos: egui::Pos2,
    pub(in crate::render) align: egui::Align2,
    pub(in crate::render) above_gate: bool,
    pub(in crate::render) outline_with_background: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AngleLabelLayout {
    above_gate: bool,
    outline_with_background: bool,
}

pub(super) fn draw_phase_connectors_and_labels(
    app: &QniApp,
    painter: &egui::Painter,
    metrics: &LayoutMetrics,
    colors: &Colors,
    circuit_origin: egui::Pos2,
    dragging_gate_id: Option<GateId>,
) {
    let render_columns = ColumnAnalysis::from_gates(&app.placed_gates, |gate| {
        gate_slot_index_for_render(gate, metrics, dragging_gate_id)
    });
    draw_phase_phase_connectors(&render_columns, painter, metrics, colors, circuit_origin);
    draw_parametric_angle_labels(
        app,
        &render_columns,
        painter,
        metrics,
        colors,
        circuit_origin,
        dragging_gate_id,
    );
    draw_flag_labels(
        app,
        &render_columns,
        painter,
        metrics,
        colors,
        circuit_origin,
        dragging_gate_id,
    );
}

fn draw_flag_labels(
    app: &QniApp,
    render_columns: &ColumnAnalysis<'_>,
    painter: &egui::Painter,
    metrics: &LayoutMetrics,
    colors: &Colors,
    circuit_origin: egui::Pos2,
    dragging_gate_id: Option<GateId>,
) {
    // `aliceX` above `Measure>aliceX` and `if aliceX` above `X<aliceX`, with
    // the same top / outlined top / bottom placement rules as angle labels.
    for gate in &app.placed_gates {
        let Some(label) = flag_label_info(
            gate,
            render_columns,
            metrics,
            circuit_origin,
            dragging_gate_id,
        ) else {
            continue;
        };
        draw_flag_label(painter, &label, colors);
    }
}

#[derive(Clone, Debug, PartialEq)]
struct FlagLabelInfo {
    prefix: Option<&'static str>,
    name: String,
    pos: egui::Pos2,
    align: egui::Align2,
    above_gate: bool,
    outline_with_background: bool,
}

fn flag_label_info(
    gate: &PlacedGate,
    render_columns: &ColumnAnalysis<'_>,
    metrics: &LayoutMetrics,
    circuit_origin: egui::Pos2,
    dragging_gate_id: Option<GateId>,
) -> Option<FlagLabelInfo> {
    let flag = gate.flag.as_ref()?;
    let center = circuit_origin + gate.pos.to_vec2() + egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0);
    // A measurement never joins a control connector (qni measurements are not
    // controllable), so its label always sits on top.
    let connection_sides = if gate.kind == GateKind::Measurement {
        ConnectionSides::default()
    } else {
        phase_render_column(render_columns, gate, metrics, dragging_gate_id)
            .map(|column| control_connection_sides(column, gate))
            .unwrap_or_default()
    };
    let layout = angle_label_layout(connection_sides);
    // qni's `-mt-3.5 leading-3` leaves 4px between the label ink and the
    // gate; the angle label anchor already lands the egui galley there.
    let (pos, align) = angle_label_position(center, layout);
    Some(FlagLabelInfo {
        prefix: flag.label_prefix(),
        name: flag.name().as_str().to_owned(),
        pos,
        align,
        above_gate: layout.above_gate,
        outline_with_background: layout.outline_with_background,
    })
}

fn draw_flag_label(painter: &egui::Painter, label: &FlagLabelInfo, colors: &Colors) {
    let layout = |text: &str, color: egui::Color32| {
        painter.layout_job(egui::text::LayoutJob::single_section(
            text.to_owned(),
            egui::TextFormat {
                // text-xs (12 px), the same font as the angle labels.
                font_id: egui::FontId::monospace(ANGLE_LABEL_FONT_SIZE),
                extra_letter_spacing: FLAG_LABEL_TRACKING,
                color,
                ..Default::default()
            },
        ))
    };
    let mut passes = Vec::new();
    if label.outline_with_background {
        passes.extend(
            ANGLE_LABEL_OUTLINE_OFFSETS.map(|(dx, dy)| (egui::vec2(dx, dy), colors.background)),
        );
    }
    passes.push((egui::Vec2::ZERO, colors.text_strong));
    for (offset, color) in passes {
        let prefix = label.prefix.map(|prefix| layout(prefix, color));
        let name = layout(&label.name, color);
        let prefix_width = prefix
            .as_ref()
            .map_or(0.0, |prefix| prefix.size().x + FLAG_LABEL_IF_GAP);
        let size = egui::vec2(prefix_width + name.size().x, name.size().y);
        let rect = label.align.anchor_size(label.pos + offset, size);
        if let Some(prefix) = prefix {
            painter.galley(rect.min, prefix, color);
        }
        painter.galley(rect.min + egui::vec2(prefix_width, 0.0), name, color);
    }
}

fn draw_phase_phase_connectors(
    render_columns: &ColumnAnalysis<'_>,
    painter: &egui::Painter,
    metrics: &LayoutMetrics,
    colors: &Colors,
    circuit_origin: egui::Pos2,
) {
    // Phase-Phase connector. qni's
    // `circuit-step-element.ts::updatePhasePhaseConnections` (:566-602)
    // draws a connector between same-angle Phase gates in the same column.
    // Each connected group is one multi-controlled phase (CPHASE / CCPHASE)
    // in the simulation, so the buckets use the same `PhaseGroupKey`.
    for column in render_columns.columns() {
        let mut groups: HashMap<PhaseGroupKey<'_>, Vec<egui::Pos2>> = HashMap::new();
        for gate in column.gates() {
            let Some(key) = PhaseGroupKey::of(gate) else {
                continue;
            };
            let center =
                circuit_origin + gate.pos.to_vec2() + egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0);
            groups.entry(key).or_default().push(center);
        }
        for points in groups.values() {
            if points.len() < 2 {
                continue;
            }
            let mut min_y = f32::INFINITY;
            let mut max_y = f32::NEG_INFINITY;
            for point in points {
                min_y = min_y.min(point.y);
                max_y = max_y.max(point.y);
            }
            // Slot-center anchored — same rationale as the control connectors.
            let x = circuit_origin.x + metrics.slot_centers[column.slot];
            draw_vertical_connector(painter, x, min_y, max_y, colors.box_fill);
        }
    }
}

fn draw_parametric_angle_labels(
    app: &QniApp,
    render_columns: &ColumnAnalysis<'_>,
    painter: &egui::Painter,
    metrics: &LayoutMetrics,
    colors: &Colors,
    circuit_origin: egui::Pos2,
    dragging_gate_id: Option<GateId>,
) {
    // Angle labels for parametric gates. qni puts the angle text near the gate
    // body and draws a white text-shadow when the dropzone has both
    // `data-connect-top` and `data-connect-bottom`
    // (`packages/elements/css/qni.css`, `.operation-angleable`). We mirror that
    // with the circuit background (Flexoki bg-2 via `colors.background`) so
    // labels stay legible over vertical connectors.
    for gate in &app.placed_gates {
        if app
            .angle_editor
            .as_ref()
            .is_some_and(|editor| editor.gate_id == gate.id)
        {
            continue;
        }
        let Some(label) = parametric_angle_label_info(
            gate,
            render_columns,
            metrics,
            circuit_origin,
            dragging_gate_id,
        ) else {
            continue;
        };
        draw_angle_label(
            painter,
            label.pos,
            label.align,
            &label.text,
            colors,
            label.outline_with_background,
        );
    }
}

pub(in crate::render) fn parametric_angle_label_info(
    gate: &PlacedGate,
    render_columns: &ColumnAnalysis<'_>,
    metrics: &LayoutMetrics,
    circuit_origin: egui::Pos2,
    dragging_gate_id: Option<GateId>,
) -> Option<AngleLabelInfo> {
    let angle = parametric_angle(gate)?;
    let text = angle.label();
    let center = circuit_origin + gate.pos.to_vec2() + egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0);
    let connection_sides = phase_render_column(render_columns, gate, metrics, dragging_gate_id)
        .map(|column| angle_label_connection_sides(column, gate))
        .unwrap_or_default();
    let layout = angle_label_layout(connection_sides);
    let (pos, align) = angle_label_position(center, layout);
    Some(AngleLabelInfo {
        gate_id: gate.id,
        text,
        pos,
        align,
        above_gate: layout.above_gate,
        outline_with_background: layout.outline_with_background,
    })
}

pub(in crate::render) fn angle_label_interaction_rect(label: &AngleLabelInfo) -> egui::Rect {
    let top = if label.above_gate {
        label.pos.y - ANGLE_LABEL_ROW_HEIGHT
    } else {
        label.pos.y
    };
    egui::Rect::from_min_size(
        egui::pos2(label.pos.x - GATE_SIZE * 0.5, top),
        egui::vec2(GATE_SIZE, ANGLE_LABEL_ROW_HEIGHT),
    )
}

pub(in crate::render) fn angle_underline_segment(label: &AngleLabelInfo) -> [egui::Pos2; 2] {
    let y = angle_underline_y(label);
    [
        egui::pos2(label.pos.x - GATE_SIZE * 0.5, y),
        egui::pos2(label.pos.x + GATE_SIZE * 0.5, y),
    ]
}

pub(in crate::render) fn reserve_angle_label_outline(
    painter: &egui::Painter,
    label: &AngleLabelInfo,
) -> Option<[egui::layers::ShapeIdx; ANGLE_LABEL_OUTLINE_OFFSETS.len()]> {
    label
        .outline_with_background
        .then(|| ANGLE_LABEL_OUTLINE_OFFSETS.map(|_| painter.add(egui::Shape::Noop)))
}

pub(in crate::render) fn paint_reserved_angle_label_outline(
    painter: &egui::Painter,
    colors: &Colors,
    text: &str,
    galley_pos: egui::Pos2,
    text_clip_rect: egui::Rect,
    outline_slots: Option<[egui::layers::ShapeIdx; ANGLE_LABEL_OUTLINE_OFFSETS.len()]>,
) {
    let Some(outline_slots) = outline_slots else {
        return;
    };
    let font_id = egui::FontId::monospace(ANGLE_LABEL_FONT_SIZE);
    let galley = painter.layout_no_wrap(text.to_owned(), font_id, colors.background);
    let clipped_painter = painter.with_clip_rect(text_clip_rect);
    for (slot, (dx, dy)) in outline_slots.into_iter().zip(ANGLE_LABEL_OUTLINE_OFFSETS) {
        clipped_painter.set(
            slot,
            egui::Shape::galley(
                galley_pos + egui::vec2(dx, dy),
                galley.clone(),
                colors.background,
            ),
        );
    }
}

fn angle_underline_y(label: &AngleLabelInfo) -> f32 {
    angle_label_interaction_rect(label).bottom() - ANGLE_UNDERLINE_BOTTOM_INSET
}

fn angle_label_layout(connection_sides: ConnectionSides) -> AngleLabelLayout {
    // Mirrors qni's `.operation-angleable` placement rules:
    //   no top connection        → top label
    //   top + bottom connection  → top label with background outline
    //   top-only connection      → bottom label
    AngleLabelLayout {
        above_gate: !connection_sides.top || connection_sides.bottom,
        outline_with_background: connection_sides.top && connection_sides.bottom,
    }
}

fn angle_label_position(
    gate_center: egui::Pos2,
    layout: AngleLabelLayout,
) -> (egui::Pos2, egui::Align2) {
    if layout.above_gate {
        (
            egui::pos2(
                gate_center.x,
                gate_center.y - GATE_SIZE / 2.0 + ANGLE_LABEL_TOP_OFFSET,
            ),
            egui::Align2::CENTER_BOTTOM,
        )
    } else {
        (
            egui::pos2(
                gate_center.x,
                gate_center.y + GATE_SIZE / 2.0 + ANGLE_LABEL_BOTTOM_GAP,
            ),
            egui::Align2::CENTER_TOP,
        )
    }
}

fn draw_angle_label(
    painter: &egui::Painter,
    pos: egui::Pos2,
    align: egui::Align2,
    text: &str,
    colors: &Colors,
    outline_with_background: bool,
) {
    // text-xs (12 px) — Tailwind. Matches the popup body font so labels
    // feel like they belong to the same typographic system.
    let font_id = egui::FontId::monospace(ANGLE_LABEL_FONT_SIZE);
    if outline_with_background {
        for (dx, dy) in ANGLE_LABEL_OUTLINE_OFFSETS {
            painter.text(
                pos + egui::vec2(dx, dy),
                align,
                text,
                font_id.clone(),
                colors.background,
            );
        }
    }
    painter.text(pos, align, text, font_id, colors.text_strong);
}

fn phase_render_column<'columns, 'gates>(
    render_columns: &'columns ColumnAnalysis<'gates>,
    gate: &PlacedGate,
    metrics: &LayoutMetrics,
    dragging_gate_id: Option<GateId>,
) -> Option<&'columns AnalyzedColumn<'gates>> {
    let slot = gate_slot_index_for_render(gate, metrics, dragging_gate_id)?;
    render_columns
        .columns()
        .iter()
        .find(|column| column.slot == slot)
}

fn parametric_angle(gate: &PlacedGate) -> Option<ParametricAngle> {
    if !matches!(
        gate.kind,
        GateKind::Phase | GateKind::Rx | GateKind::Ry | GateKind::Rz
    ) {
        return None;
    }
    Some(gate.angle.unwrap_or_default())
}

#[cfg(test)]
fn parametric_angle_label_text(gate: &PlacedGate) -> Option<String> {
    parametric_angle(gate).map(|angle| angle.label())
}

fn angle_label_connection_sides(column: &AnalyzedColumn<'_>, gate: &PlacedGate) -> ConnectionSides {
    let mut sides = phase_phase_connection_sides(column, gate);
    sides.include(control_connection_sides(column, gate));
    sides
}

fn phase_phase_connection_sides(column: &AnalyzedColumn<'_>, gate: &PlacedGate) -> ConnectionSides {
    let Some(key) = PhaseGroupKey::of(gate) else {
        return ConnectionSides::default();
    };
    let mut sides = ConnectionSides::default();
    for other in column.gates() {
        if other.id == gate.id || PhaseGroupKey::of(other) != Some(key) {
            continue;
        }
        sides.include_wire(gate.wire, other.wire);
    }
    sides
}

fn control_connection_sides(column: &AnalyzedColumn<'_>, gate: &PlacedGate) -> ConnectionSides {
    let controls = column
        .gates()
        .iter()
        .filter(|other| matches!(other.kind, GateKind::Control | GateKind::AntiControl))
        .count();
    if controls == 0 {
        return ConnectionSides::default();
    }
    let targets = column
        .gates()
        .iter()
        .filter(|other| {
            !matches!(
                other.kind,
                GateKind::Control | GateKind::AntiControl | GateKind::Swap
            )
        })
        .count();
    if targets == 0 && controls < 2 {
        return ConnectionSides::default();
    }

    let mut sides = ConnectionSides::default();
    for other in column.gates() {
        if other.id == gate.id || other.kind == GateKind::Swap {
            continue;
        }
        sides.include_wire(gate.wire, other.wire);
    }
    sides
}

#[cfg(test)]
mod tests {
    use super::*;

    fn angle(value: &str) -> ParametricAngle {
        ParametricAngle::parse_qni(value).expect("angle should parse")
    }

    #[test]
    fn phase_label_uses_default_for_missing_angle() {
        let gate = PlacedGate::new(
            crate::app::GateId::from_u32(1),
            GateKind::Phase,
            crate::app::CircuitColumnIndex::new(0),
            crate::app::WireIndex::new(0),
            crate::gates::GateSpan::SINGLE,
            None,
        );

        assert_eq!(parametric_angle_label_text(&gate), Some("π/2".to_owned()));
    }

    #[test]
    fn phase_label_keeps_explicit_angle() {
        let gate = PlacedGate::new(
            crate::app::GateId::from_u32(1),
            GateKind::Phase,
            crate::app::CircuitColumnIndex::new(0),
            crate::app::WireIndex::new(0),
            crate::gates::GateSpan::SINGLE,
            Some(angle("2π/3")),
        );

        assert_eq!(parametric_angle_label_text(&gate), Some("2π/3".to_owned()));
    }

    #[test]
    fn rx_label_uses_default_for_missing_angle() {
        let gate = PlacedGate::new(
            crate::app::GateId::from_u32(1),
            GateKind::Rx,
            crate::app::CircuitColumnIndex::new(0),
            crate::app::WireIndex::new(0),
            crate::gates::GateSpan::SINGLE,
            None,
        );

        assert_eq!(parametric_angle_label_text(&gate), Some("π/2".to_owned()));
    }

    #[test]
    fn ry_label_uses_default_for_missing_angle() {
        let gate = PlacedGate::new(
            crate::app::GateId::from_u32(1),
            GateKind::Ry,
            crate::app::CircuitColumnIndex::new(0),
            crate::app::WireIndex::new(0),
            crate::gates::GateSpan::SINGLE,
            None,
        );

        assert_eq!(parametric_angle_label_text(&gate), Some("π/2".to_owned()));
    }

    #[test]
    fn ry_label_keeps_explicit_angle() {
        let gate = PlacedGate::new(
            crate::app::GateId::from_u32(1),
            GateKind::Ry,
            crate::app::CircuitColumnIndex::new(0),
            crate::app::WireIndex::new(0),
            crate::gates::GateSpan::SINGLE,
            Some(angle("π/4")),
        );

        assert_eq!(parametric_angle_label_text(&gate), Some("π/4".to_owned()));
    }

    #[test]
    fn rz_label_uses_default_for_missing_angle() {
        let gate = PlacedGate::new(
            crate::app::GateId::from_u32(1),
            GateKind::Rz,
            crate::app::CircuitColumnIndex::new(0),
            crate::app::WireIndex::new(0),
            crate::gates::GateSpan::SINGLE,
            None,
        );

        assert_eq!(parametric_angle_label_text(&gate), Some("π/2".to_owned()));
    }

    #[test]
    fn middle_phase_label_gets_background_outline() {
        let layout = angle_label_layout(ConnectionSides {
            top: true,
            bottom: true,
        });

        assert!(layout.outline_with_background);
    }

    #[test]
    fn topmost_phase_label_does_not_get_background_outline() {
        let layout = angle_label_layout(ConnectionSides {
            top: false,
            bottom: true,
        });

        assert!(!layout.outline_with_background);
    }

    #[test]
    fn bottommost_phase_label_uses_bottom_placement() {
        let layout = angle_label_layout(ConnectionSides {
            top: true,
            bottom: false,
        });

        assert!(!layout.above_gate);
    }

    #[test]
    fn phase_with_phase_above_and_control_below_gets_both_connection_sides() {
        let gates = vec![
            PlacedGate::new(
                crate::app::GateId::from_u32(1),
                GateKind::Phase,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(0),
                crate::gates::GateSpan::SINGLE,
                Some(angle("π/2")),
            ),
            PlacedGate::new(
                crate::app::GateId::from_u32(2),
                GateKind::Phase,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(1),
                crate::gates::GateSpan::SINGLE,
                Some(angle("4π/8")),
            ),
            PlacedGate::new(
                crate::app::GateId::from_u32(3),
                GateKind::Control,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(2),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
        ];
        let analysis = ColumnAnalysis::from_gates(&gates, |gate| Some(gate.column.as_usize()));

        assert_eq!(
            angle_label_connection_sides(&analysis.columns()[0], &gates[1]),
            ConnectionSides {
                top: true,
                bottom: true,
            }
        );
    }

    #[test]
    fn underline_matches_prototype_row_inset() {
        let label = AngleLabelInfo {
            gate_id: crate::app::GateId::from_u32(1),
            text: "π/2".to_owned(),
            pos: egui::pos2(80.0, 60.0),
            align: egui::Align2::CENTER_BOTTOM,
            above_gate: true,
            outline_with_background: false,
        };

        assert_eq!(angle_underline_y(&label), 57.5);
    }

    #[test]
    fn top_label_uses_lower_gate_biased_anchor() {
        let gate_center = egui::pos2(10.0, 80.0);
        let layout = angle_label_layout(ConnectionSides {
            top: true,
            bottom: true,
        });

        assert_eq!(angle_label_position(gate_center, layout).0.y, 60.0);
    }

    fn flagged_column(json: &str) -> Vec<PlacedGate> {
        crate::url_circuit::parse_circuit_json(json).gates
    }

    fn flag_label(gates: &[PlacedGate], index: usize) -> Option<FlagLabelInfo> {
        let metrics = crate::layout::layout_metrics(800.0, 3, 4);
        let analysis = ColumnAnalysis::from_gates(gates, |gate| Some(gate.column.as_usize()));
        flag_label_info(&gates[index], &analysis, &metrics, egui::Pos2::ZERO, None)
    }

    #[test]
    fn measurement_flag_label_shows_the_flag_name() {
        let gates = flagged_column(r#"{"cols":[["Measure>aliceX"]]}"#);

        assert_eq!(
            flag_label(&gates, 0).map(|label| label.name),
            Some("aliceX".to_owned())
        );
    }

    #[test]
    fn flag_label_shares_the_angle_label_anchor() {
        let gates = flagged_column(r#"{"cols":[["X<aliceX"]]}"#);
        let gate_top = gates[0].pos.y;

        assert_eq!(
            flag_label(&gates, 0).map(|label| label.pos.y),
            Some(gate_top)
        );
    }

    #[test]
    fn bottom_target_flag_label_moves_below_the_gate() {
        let gates = flagged_column(r#"{"cols":[["•","X<a"]]}"#);

        assert_eq!(
            flag_label(&gates, 1).map(|label| label.above_gate),
            Some(false)
        );
    }

    #[test]
    fn measurement_flag_label_ignores_column_controls() {
        let gates = flagged_column(r#"{"cols":[["•","Measure>a"]]}"#);

        assert_eq!(
            flag_label(&gates, 1).map(|label| label.above_gate),
            Some(true)
        );
    }

    #[test]
    fn plain_gate_has_no_flag_label() {
        let gates = flagged_column(r#"{"cols":[["X"]]}"#);

        assert_eq!(flag_label(&gates, 0), None);
    }

    fn phase_sides(json: &str, flags: &[(usize, &str)]) -> ConnectionSides {
        let mut gates = flagged_column(json);
        for (index, name) in flags {
            gates[*index].flag = Some(crate::gates::GateFlag::If(
                crate::gates::FlagName::parse(name).expect("flag name"),
            ));
        }
        let analysis = ColumnAnalysis::from_gates(&gates, |gate| Some(gate.column.as_usize()));
        phase_phase_connection_sides(&analysis.columns()[0], &gates[0])
    }

    #[test]
    fn same_angle_phases_connect() {
        assert_eq!(
            phase_sides(r#"{"cols":[["P(π_4)","P(π_4)"]]}"#, &[]),
            ConnectionSides {
                top: false,
                bottom: true,
            }
        );
    }

    #[test]
    fn different_angle_phases_do_not_connect() {
        assert_eq!(
            phase_sides(r#"{"cols":[["P(π_4)","P(π_2)"]]}"#, &[]),
            ConnectionSides::default()
        );
    }

    #[test]
    fn phases_with_different_conditions_do_not_connect() {
        assert_eq!(
            phase_sides(r#"{"cols":[["P(π_4)","P(π_4)"]]}"#, &[(0, "a"), (1, "b")]),
            ConnectionSides::default()
        );
    }
}
