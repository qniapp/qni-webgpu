//! Palette, palette tooltip, and drag-preview drawing.

use eframe::egui;

use crate::app::QniApp;
use crate::colors::Colors;
use crate::constants::{
    GATE_SIZE, PALETTE_CORNER_RADIUS, PALETTE_PADDING_X, PALETTE_PADDING_Y, PALETTE_ROW_Y,
    PALETTE_SIZE,
};
use crate::gates::{GateKind, PALETTE_GATES};
use crate::icons::{draw_bloch_vector, draw_drag_gate_body, draw_gate_body};
use crate::layout::{palette_gate_local_pos, palette_layout};

impl QniApp {
    pub(crate) fn draw_palette(&self, painter: &egui::Painter, rect: egui::Rect, colors: &Colors) {
        let layout = palette_layout();
        let palette_start_x = rect.width() / 2.0 - layout.total_width / 2.0;
        let palette_rect = egui::Rect::from_min_size(
            rect.min
                + egui::vec2(
                    palette_start_x - PALETTE_PADDING_X,
                    PALETTE_ROW_Y - PALETTE_PADDING_Y,
                ),
            egui::vec2(
                layout.total_width + PALETTE_PADDING_X * 2.0,
                layout.total_height + PALETTE_PADDING_Y * 2.0,
            ),
        );
        let palette_corner = egui::CornerRadius::same(PALETTE_CORNER_RADIUS);
        let shadow = egui::epaint::Shadow {
            offset: [0, 6],
            blur: 16,
            spread: 0,
            color: egui::Color32::from_rgba_unmultiplied(0, 0, 0, 25),
        };
        painter.add(egui::Shape::Rect(
            shadow.as_shape(palette_rect, palette_corner),
        ));
        painter.rect_filled(palette_rect, palette_corner, colors.surface);

        let palette_origin = rect.min + egui::vec2(palette_start_x, PALETTE_ROW_Y);
        for (index, gate) in PALETTE_GATES.iter().enumerate() {
            let Some(local) = palette_gate_local_pos(index, &layout) else {
                continue;
            };
            let gate_rect = egui::Rect::from_min_size(
                palette_origin + local.to_vec2(),
                egui::vec2(PALETTE_SIZE, PALETTE_SIZE),
            );
            if self.hovered_palette_index == Some(index) {
                let hover_outer = gate_rect.expand(4.0);
                let hover_inner = gate_rect.expand(2.0);
                painter.rect_filled(hover_outer, egui::CornerRadius::same(10), colors.box_border);
                painter.rect_filled(hover_inner, egui::CornerRadius::same(8), colors.background);
            }
            draw_gate_body(painter, gate_rect, *gate, colors);
            if *gate == GateKind::BlochDisplay {
                // Palette has no associated state: render qni's d=0 blue center dot.
                draw_bloch_vector(painter, gate_rect, [0.0, 0.0, 0.0], colors);
            }
        }
    }

    /// Hover tooltip painted over the palette: a paper card with the
    /// gate's full name, qni-style description paragraphs, and a mini
    /// transformation diagram (input amplitudes → gate → output
    /// amplitudes). Anchored below the hovered palette button, clamped
    /// to the screen rect. No-op when nothing is hovered or while a
    /// gate drag is in progress.
    ///
    /// Chrome matches the state panel (paper bg + ui-2 1 px border +
    /// soft shadow). Typography follows the Tailwind scale: title
    /// text-sm (14 px) in tx, description text-xs (12 px) in tx-2.
    pub(crate) fn draw_palette_tooltip(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        colors: &Colors,
    ) {
        let Some(index) = self.hovered_palette_index else {
            return;
        };
        if self.dragging.is_some() {
            return;
        }
        let Some(&gate) = PALETTE_GATES.get(index) else {
            return;
        };
        let layout = palette_layout();
        let Some(local) = palette_gate_local_pos(index, &layout) else {
            return;
        };
        let palette_start_x = rect.width() / 2.0 - layout.total_width / 2.0;
        let palette_origin = rect.min + egui::vec2(palette_start_x, PALETTE_ROW_Y);
        let gate_rect = egui::Rect::from_min_size(
            palette_origin + local.to_vec2(),
            egui::vec2(PALETTE_SIZE, PALETTE_SIZE),
        );

        let info = gate.info();

        // ── Text layout — sizes mirror qni's `.tooltip-*` utilities:
        //     title  = `text-lg`  (18 px) `font-bold` tx — qni's
        //              `.tooltip-heading`. We can't render true bold
        //              without bundling a bold font; rely on size +
        //              colour contrast for hierarchy.
        //     para   = `text-sm`  (14 px) tx-2 — `.tooltip-subheading`.
        let title_galley = painter.layout_no_wrap(
            info.name.to_owned(),
            egui::FontId::proportional(18.0),
            colors.text_strong,
        );
        let desc_galleys: Vec<_> = info
            .paragraphs
            .iter()
            .map(|line| {
                painter.layout_no_wrap(
                    (*line).to_owned(),
                    egui::FontId::proportional(14.0),
                    colors.text,
                )
            })
            .collect();

        // ── Diagram geometry. Sizes match qni's `QubitTransitionComponent`:
        //   * QubitCircle = `h-8 w-8`        → 32 × 32 px
        //   * qpu-operation-sm = `1.5rem`    → 24 × 24 px gate body
        //   * arrow_start / arrow_end SVG    → 12 × 24 px each side
        //   * space-x-2 between groups       → 8 px
        // Per row layout:
        //   [amps_from (2 × 32px circle + 8px gap)]
        //   [12px wire][24px gate][12px wire ending in 6px chevron]
        //   [amps_to (same shape)]
        const CIRCLE: f32 = 32.0;
        const CIRCLE_GAP: f32 = 8.0;
        const SECTION_GAP: f32 = 8.0;
        const WIRE: f32 = 12.0;
        const ARROWHEAD: f32 = 6.0;
        const GATE_BODY: f32 = 24.0;
        const ROW_GAP: f32 = 8.0;
        let amps_w = CIRCLE * 2.0 + CIRCLE_GAP;
        // The arrowhead chevron is drawn over the last 6 px of the right
        // wire (matches qni's arrow_end SVG where the chevron tip ends
        // at x=11.6 within a 12 px wire). So the connector width is
        // simply 12 + 24 + 12 — ARROWHEAD is the chevron length used
        // during drawing, not a separate horizontal slot.
        let conn_w = WIRE + GATE_BODY + WIRE;
        let diagram_w = amps_w + SECTION_GAP + conn_w + SECTION_GAP + amps_w;
        let row_h = CIRCLE + 4.0; // room for the basis label tucked into the bottom-right
        let diagram_h = if info.transitions.is_empty() {
            0.0
        } else {
            let n = info.transitions.len() as f32;
            n * row_h + (n - 1.0).max(0.0) * ROW_GAP
        };

        // ── Card sizing — Tailwind values straight from qni's tooltip
        //     theme: `px-4 py-3 rounded-lg`, `.tooltip-subheading-first`
        //     `mt-1`, `.tooltip-subheading-second-and-subsequent`
        //     `mt-0.5`, `.tooltip-body` `mt-4`.
        let pad_x = 16.0_f32; // px-4
        let pad_y = 12.0_f32; // py-3
        let title_gap = 4.0_f32; // mt-1 between title and first paragraph
        let para_gap = 2.0_f32; // mt-0.5 between paragraphs
        let diagram_gap = 16.0_f32; // .tooltip-body { mt-4 }
        let desc_block_h: f32 = if desc_galleys.is_empty() {
            0.0
        } else {
            desc_galleys.iter().map(|g| g.size().y).sum::<f32>()
                + para_gap * (desc_galleys.len() as f32 - 1.0)
        };
        let desc_w = desc_galleys
            .iter()
            .map(|g| g.size().x)
            .fold(0.0_f32, f32::max);
        let content_w = title_galley.size().x.max(desc_w).max(diagram_w);
        let mut content_h = title_galley.size().y;
        if desc_block_h > 0.0 {
            content_h += title_gap + desc_block_h;
        }
        if diagram_h > 0.0 {
            content_h += diagram_gap + diagram_h;
        }
        let card_size = egui::vec2(content_w + pad_x * 2.0, content_h + pad_y * 2.0);

        // ── Anchor below the gate, clamped to the screen rect.
        let anchor = egui::pos2(gate_rect.left(), gate_rect.bottom() + 8.0);
        let max_left = rect.right() - card_size.x - 8.0;
        let max_top = rect.bottom() - card_size.y - 8.0;
        let card_min = egui::pos2(
            anchor.x.min(max_left).max(rect.left() + 8.0),
            anchor.y.min(max_top),
        );
        let card_rect = egui::Rect::from_min_size(card_min, card_size);
        let corner = egui::CornerRadius::same(8); // Tailwind rounded-lg

        let shadow = egui::epaint::Shadow {
            offset: [0, 6],
            blur: 16,
            spread: 0,
            color: egui::Color32::from_rgba_unmultiplied(0, 0, 0, 25),
        };
        painter.add(egui::Shape::Rect(shadow.as_shape(card_rect, corner)));
        painter.rect_filled(card_rect, corner, colors.surface);
        painter.rect_stroke(
            card_rect,
            corner,
            egui::Stroke::new(1.0, colors.box_border),
            egui::StrokeKind::Inside,
        );

        // ── Title.
        let title_pos = card_rect.min + egui::vec2(pad_x, pad_y);
        let title_h = title_galley.size().y;
        painter.galley(title_pos, title_galley, colors.text_strong);

        // ── Description paragraphs.
        let mut cursor_y = title_pos.y + title_h;
        if !desc_galleys.is_empty() {
            cursor_y += title_gap;
        }
        for galley in desc_galleys {
            let h = galley.size().y;
            painter.galley(egui::pos2(title_pos.x, cursor_y), galley, colors.text);
            cursor_y += h + para_gap;
        }

        // ── Diagram (one row per transition).
        if !info.transitions.is_empty() {
            // Trim trailing para_gap before the diagram block.
            let diagram_top = cursor_y
                - if info.paragraphs.is_empty() {
                    0.0
                } else {
                    para_gap
                }
                + diagram_gap;
            let diagram_left = card_rect.center().x - diagram_w / 2.0;
            for (row_idx, trans) in info.transitions.iter().enumerate() {
                let row_top = diagram_top + row_idx as f32 * (row_h + ROW_GAP);
                let row_center_y = row_top + CIRCLE / 2.0;

                // Left amplitudes (input).
                self.draw_tooltip_amps(painter, diagram_left, row_top, &trans.from, colors);
                let mut x = diagram_left + amps_w + SECTION_GAP;

                // Connector: 12 px wire → 24 px gate body → 12 px wire
                // whose last 6 px hold the arrowhead chevron (matches
                // qni's arrow_start / arrow_end SVG geometry). Both
                // wires are pulled 2 px short of the gate edges so the
                // gate sits with a small breathing-room gap on either
                // side instead of being visually fused to the line.
                const WIRE_GATE_PAD: f32 = 2.0;
                let wire_color = colors.text_strong;
                painter.line_segment(
                    [
                        egui::pos2(x, row_center_y),
                        egui::pos2(x + WIRE - WIRE_GATE_PAD, row_center_y),
                    ],
                    egui::Stroke::new(2.0, wire_color),
                );
                let gate_x = x + WIRE;
                let gate_rect_mini = egui::Rect::from_min_size(
                    egui::pos2(gate_x, row_center_y - GATE_BODY / 2.0),
                    egui::vec2(GATE_BODY, GATE_BODY),
                );
                self.draw_tooltip_mini_gate(painter, gate_rect_mini, gate, colors);
                let wire2_x = gate_x + GATE_BODY;
                let arrow_tip = egui::pos2(wire2_x + WIRE, row_center_y);
                // Line ending where the chevron starts (arrow tip −6 px),
                // starting WIRE_GATE_PAD after the gate's right edge.
                painter.line_segment(
                    [
                        egui::pos2(wire2_x + WIRE_GATE_PAD, row_center_y),
                        egui::pos2(arrow_tip.x - ARROWHEAD + 1.0, row_center_y),
                    ],
                    egui::Stroke::new(2.0, wire_color),
                );
                let arrow_base_x = arrow_tip.x - ARROWHEAD;
                painter.add(egui::Shape::convex_polygon(
                    vec![
                        arrow_tip,
                        egui::pos2(arrow_base_x, row_center_y - 4.0),
                        egui::pos2(arrow_base_x, row_center_y + 4.0),
                    ],
                    wire_color,
                    egui::Stroke::NONE,
                ));
                x = arrow_tip.x + SECTION_GAP;

                // Right amplitudes (output).
                self.draw_tooltip_amps(painter, x, row_top, &trans.to, colors);
            }
        }
    }

    /// Render a 2-amplitude row (one `[Amp; 2]`) at the given top-left
    /// position. Each amp = outline circle + filled disk sized by
    /// |amp|² + phase-needle line, with the basis label `|0⟩` / `|1⟩`
    /// tucked into the bottom-right corner (qni convention).
    fn draw_tooltip_amps(
        &self,
        painter: &egui::Painter,
        left_x: f32,
        top_y: f32,
        amps: &[crate::gates::Amp; 2],
        colors: &Colors,
    ) {
        // 32 × 32 px — same size as qni's `qubit-circle` (h-8 w-8).
        const CIRCLE: f32 = 32.0;
        const CIRCLE_GAP: f32 = 8.0;
        for (basis, amp) in amps.iter().enumerate() {
            let center = egui::pos2(
                left_x + CIRCLE / 2.0 + basis as f32 * (CIRCLE + CIRCLE_GAP),
                top_y + CIRCLE / 2.0,
            );
            let prob = amp.probability().clamp(0.0, 1.0);
            let is_zero = prob < 1e-6;
            let outline = if is_zero {
                colors.state_outline_zero
            } else {
                colors.state_outline
            };
            painter.circle_stroke(center, CIRCLE / 2.0, egui::Stroke::new(1.5, outline));
            if !is_zero {
                let inner_r = (CIRCLE / 2.0) * prob.sqrt();
                painter.circle_filled(center, inner_r, colors.state_fill);
                let phase = amp.phase();
                let tip = egui::pos2(
                    center.x + phase.sin() * (CIRCLE / 2.0),
                    center.y - phase.cos() * (CIRCLE / 2.0),
                );
                painter.line_segment([center, tip], egui::Stroke::new(2.0, colors.state_needle));
            }
            // Basis label `|0⟩` / `|1⟩` tucked tight against the
            // circle's bottom-right edge (qni convention). The anchor
            // is the label's top-left, placed at ~5 o'clock just inside
            // the circle's outline so the label's bounding box hugs the
            // disk without floating off to the side.
            let label = if basis == 0 { "|0⟩" } else { "|1⟩" };
            painter.text(
                egui::pos2(center.x + CIRCLE / 2.0 - 7.0, center.y + CIRCLE / 2.0 - 6.0),
                egui::Align2::LEFT_TOP,
                label,
                egui::FontId::monospace(10.0),
                colors.text,
            );
        }
    }

    /// Mini gate body (24 px) for the tooltip diagram — matches qni's
    /// `qpu-operation-sm` (1.5rem). Delegates to the shared
    /// `draw_gate_body` so the icon glyph is identical to what the
    /// palette renders (Phase = `Ø` circle, X = filled disk, …), just
    /// scaled down. Without this every gate would fall back to its
    /// short text `label()` (`P` / `X` / `Ry` / …) and the diagram
    /// wouldn't visually match the palette icon the user is hovering.
    fn draw_tooltip_mini_gate(
        &self,
        painter: &egui::Painter,
        gate_rect: egui::Rect,
        kind: GateKind,
        colors: &Colors,
    ) {
        draw_gate_body(painter, gate_rect, kind, colors);
    }

    pub(crate) fn draw_drag_preview(
        &self,
        painter: &egui::Painter,
        content_rect: egui::Rect,
        colors: &Colors,
        dragging_gate_id: u32,
        scroll_x: f32,
    ) {
        let Some(gate) = self
            .placed_gates
            .iter()
            .find(|gate| gate.id == dragging_gate_id)
        else {
            return;
        };
        // Same convention as draw_circuit — gate.pos is in circuit
        // space, so we shift the content_rect origin left by the scroll
        // offset before placing the drag preview.
        let circuit_origin = content_rect.min - egui::vec2(scroll_x, 0.0);
        let gate_rect = egui::Rect::from_min_size(
            circuit_origin + gate.pos.to_vec2(),
            egui::vec2(GATE_SIZE, GATE_SIZE),
        );
        draw_drag_gate_body(painter, gate_rect, gate.kind, colors);
        if gate.kind == GateKind::BlochDisplay {
            // While dragging the gate isn't snapped, so we can't compute a
            // Bloch vector. Render the qni d=0 blue dot at the sphere center.
            draw_bloch_vector(painter, gate_rect, [0.0, 0.0, 0.0], colors);
        }
    }
}
