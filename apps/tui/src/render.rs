use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Widget};

use crate::layout::{
    circuit_layout, column_line_x, insertion_snap_rect, layout_regions, palette_items, start_line_x,
};
use crate::model::{
    apply_gates_to_zero_limit, default_phase_value, ensure_slots, qubit_count, AppState, Complex,
    Gate,
};
use crate::{
    GATE_BOX_HEIGHT, GATE_BOX_WIDTH, GATE_DRAW_HEIGHT, PALETTE_GAP, PALETTE_LABEL,
    SHADOW_OUTSET, UI_BACKGROUND,
};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct DragVisual {
    pub gate: Gate,
    pub x: u16,
    pub y: u16,
}

fn gate_theme(gate: Gate) -> (Color, Color, Color, Color) {
    let (r, g, b) = match gate {
        Gate::Measure => (170, 90, 200),
        _ => (16, 185, 129),
    };
    let background = Color::Rgb(r, g, b);
    let highlight = brighten(r, g, b, 60);
    let shadow = darken(r, g, b, 60);
    let text = Color::White;
    (text, background, highlight, shadow)
}

fn brighten(r: u8, g: u8, b: u8, delta: u8) -> Color {
    Color::Rgb(r.saturating_add(delta), g.saturating_add(delta), b.saturating_add(delta))
}

fn darken(r: u8, g: u8, b: u8, delta: u8) -> Color {
    Color::Rgb(r.saturating_sub(delta), g.saturating_sub(delta), b.saturating_sub(delta))
}

pub(crate) fn draw_gate_box(
    buffer: &mut Buffer,
    rect: Rect,
    gate: Gate,
    measure_value: Option<u8>,
    phase_label: Option<&str>,
    phase_edit_active: bool,
) {
    if rect.width < GATE_BOX_WIDTH || rect.height < GATE_DRAW_HEIGHT {
        return;
    }
    let (text, background, highlight, shadow) = gate_theme(gate);
    if gate == Gate::Control {
        let style = Style::default()
            .fg(background)
            .bg(UI_BACKGROUND)
            .add_modifier(Modifier::BOLD);
        let mid_y = rect.y.saturating_add(rect.height / 2);
        let mid_x = rect.x.saturating_add(rect.width / 2);
        buffer.set_string(mid_x, mid_y, "●", style);
        return;
    }
    if gate == Gate::Measure {
        let base_style = Style::default().fg(text).bg(background);
        let highlight_style = Style::default().fg(highlight).bg(background);
        let shadow_style = Style::default().fg(shadow).bg(background);
        let mid_y = rect.y.saturating_add(rect.height / 2);
        let mid_x = rect.x.saturating_add(rect.width / 2);
        let fill = " ".repeat(rect.width as usize);
        for offset in 0..rect.height {
            buffer.set_string(rect.x, rect.y + offset, &fill, base_style);
        }
        if rect.height > 2 {
            let top = "▔".repeat(rect.width as usize);
            buffer.set_string(rect.x, rect.y, &top, highlight_style);
        }
        if rect.height > 1 {
            let bottom = "▁".repeat(rect.width as usize);
            buffer.set_string(
                rect.x,
                rect.y + rect.height.saturating_sub(1),
                &bottom,
                shadow_style,
            );
        }
        let symbol = match measure_value {
            Some(1) => "1",
            Some(0) => "0",
            _ => "M",
        };
        buffer.set_string(mid_x, mid_y, symbol, base_style);
        return;
    }
    let base_style = Style::default().fg(text).bg(background);
    let highlight_style = Style::default().fg(highlight).bg(background);
    let shadow_style = Style::default().fg(shadow).bg(background);
    if rect.height > 2 {
        let top = "▔".repeat(rect.width as usize);
        buffer.set_string(rect.x, rect.y, &top, highlight_style);
    }
    if rect.height > 1 {
        let bottom = "▁".repeat(rect.width as usize);
        buffer.set_string(
            rect.x,
            rect.y + rect.height.saturating_sub(1),
            &bottom,
            shadow_style,
        );
    }
    if matches!(gate, Gate::Phase | Gate::Rx | Gate::Ry | Gate::Rz)
        && rect.width >= 3
        && rect.y > 0
    {
        if let Some(label) = phase_label {
            let label_width = label.chars().count() as u16;
            let label_x = rect
                .x
                .saturating_add(rect.width.saturating_sub(label_width) / 2);
            let label_style = if phase_edit_active {
                Style::default().fg(UI_BACKGROUND).bg(background)
            } else {
                Style::default().fg(background).bg(UI_BACKGROUND)
            };
            buffer.set_string(label_x, rect.y - 1, label, label_style);
        }
    }
    let gate_rect = Rect {
        x: rect.x,
        y: rect.y + SHADOW_OUTSET,
        width: rect.width,
        height: GATE_BOX_HEIGHT,
    };
    if gate == Gate::Swap {
        let swap_rect = Rect {
            x: gate_rect.x,
            y: gate_rect.y,
            width: gate_rect.width,
            height: gate_rect.height.min(3),
        };
        let fill = " ".repeat(swap_rect.width as usize);
        let mid_y = swap_rect.y.saturating_add(swap_rect.height / 2);
        buffer.set_string(swap_rect.x, mid_y, &fill, base_style);
        if swap_rect.width > 2 {
            let gap = " ".repeat(swap_rect.width.saturating_sub(2) as usize);
            let gap_style = Style::default().fg(UI_BACKGROUND).bg(UI_BACKGROUND);
            buffer.set_string(swap_rect.x + 1, swap_rect.y, &gap, gap_style);
            buffer.set_string(
                swap_rect.x + 1,
                swap_rect.y + swap_rect.height.saturating_sub(1),
                &gap,
                gap_style,
            );
        }
        let edge_style = Style::default().fg(UI_BACKGROUND).bg(UI_BACKGROUND);
        for offset in 0..swap_rect.height {
            let y = swap_rect.y + offset;
            buffer.set_string(swap_rect.x, y, " ", edge_style);
            buffer.set_string(
                swap_rect.x + swap_rect.width.saturating_sub(1),
                y,
                " ",
                edge_style,
            );
        }
        let left_x = if swap_rect.width > 2 {
            swap_rect.x + 1
        } else {
            swap_rect.x
        };
        let right_x = if swap_rect.width > 2 {
            swap_rect.x.saturating_add(swap_rect.width.saturating_sub(2))
        } else {
            swap_rect.x.saturating_add(swap_rect.width.saturating_sub(1))
        };
        let top_y = swap_rect.y;
        let bottom_y = swap_rect.y.saturating_add(swap_rect.height.saturating_sub(1));
        let tip_style = Style::default().fg(background).bg(background);
        buffer.set_string(left_x, top_y, "\\", tip_style);
        buffer.set_string(right_x, top_y, "/", tip_style);
        buffer.set_string(left_x, bottom_y, "/", tip_style);
        buffer.set_string(right_x, bottom_y, "\\", tip_style);
        let cut_style = Style::default().fg(UI_BACKGROUND).bg(background);
        buffer.set_string(left_x, mid_y, "▶", cut_style);
        buffer.set_string(right_x, mid_y, "◀", cut_style);
        return;
    }
    if gate == Gate::X {
        let full = " ".repeat(gate_rect.width as usize);
        let _inner = " ".repeat(gate_rect.width.saturating_sub(2) as usize);
        if gate_rect.height > 2 {
            for offset in 1..gate_rect.height - 1 {
                buffer.set_string(gate_rect.x, gate_rect.y + offset, &full, base_style);
            }
        }
        if gate_rect.width >= 5 && gate_rect.height >= 3 {
            let corner = Style::default().fg(UI_BACKGROUND).bg(background);
            buffer.set_string(gate_rect.x, gate_rect.y, "◤", corner);
            buffer.set_string(
                gate_rect.x + gate_rect.width - 1,
                gate_rect.y,
                "◥",
                corner,
            );
            buffer.set_string(
                gate_rect.x,
                gate_rect.y + gate_rect.height - 1,
                "◣",
                corner,
            );
            buffer.set_string(
                gate_rect.x + gate_rect.width - 1,
                gate_rect.y + gate_rect.height - 1,
                "◢",
                corner,
            );
            buffer.set_string(
                gate_rect.x + gate_rect.width / 2,
                gate_rect.y + gate_rect.height / 2,
                "+",
                Style::default()
                    .fg(text)
                    .bg(background)
                    .add_modifier(Modifier::BOLD),
            );
        }
        return;
    }
    if gate_rect.width > 2 && gate_rect.height > 2 {
        let fill = " ".repeat(gate_rect.width as usize);
        for row in 1..gate_rect.height - 1 {
            buffer.set_string(gate_rect.x, gate_rect.y + row, &fill, base_style);
        }
    }
    let label_x = if gate == Gate::SqrtX {
        gate_rect.x + gate_rect.width / 2 - 1
    } else {
        gate_rect.x + gate_rect.width / 2
    };
    let label_y = gate_rect.y + gate_rect.height / 2;
    buffer.set_string(
        label_x,
        label_y,
        gate.to_string(),
        Style::default()
            .fg(text)
            .bg(background)
            .add_modifier(Modifier::BOLD),
    );
}

pub fn render_to_buffer(state: &mut AppState, area: Rect, debug_line: Option<&str>) -> Buffer {
    render_to_buffer_with_drag(state, area, debug_line, None)
}

pub fn render_to_buffer_with_drag(
    state: &mut AppState,
    area: Rect,
    debug_line: Option<&str>,
    drag: Option<DragVisual>,
) -> Buffer {
    let mut buffer = Buffer::empty(area);
    buffer.set_style(area, Style::default().bg(UI_BACKGROUND));
    let current_qubits = qubit_count(state);
    let regions = layout_regions(area, current_qubits);
    let layout = circuit_layout(area, current_qubits);
    let counts: Vec<usize> = layout.slots.iter().map(|row| row.len()).collect();
    ensure_slots(state, &counts);
    if !PALETTE_LABEL.is_empty() {
        buffer.set_string(
            regions.palette.x,
            regions.palette.y,
            PALETTE_LABEL,
            Style::default().bg(UI_BACKGROUND),
        );
    }
    for item in palette_items(regions.palette) {
        draw_gate_box(&mut buffer, item.rect, item.gate, None, None, false);
    }

    if !regions.circuits.is_empty() {
        let palette_bottom = regions.palette.y.saturating_add(regions.palette.height);
        let separator_y = palette_bottom.saturating_add(PALETTE_GAP);
        let line = "─".repeat(regions.palette.width as usize);
        buffer.set_string(
            regions.palette.x,
            separator_y,
            line,
            Style::default().fg(Color::DarkGray).bg(UI_BACKGROUND),
        );
    }

    let wire_line = "-".repeat(layout.wire_width as usize);
    for row in 0..current_qubits {
        if let Some(wire_y) = layout.wire_rows.get(row) {
            buffer.set_string(
                regions.circuits[row].x,
                *wire_y,
                format!("q{}: {}", row, wire_line),
                Style::default().bg(UI_BACKGROUND),
            );
        }
    }

    let top = regions
        .circuits
        .first()
        .map(|rect| rect.y)
        .unwrap_or(area.y);
    let bottom = regions
        .circuits
        .last()
        .map(|rect| rect.y.saturating_add(rect.height))
        .unwrap_or(top);

    let implicit_start = state.confirmed_column.is_none()
        && !state.confirmed_start
        && state.hovered_column.is_none()
        && !state.hovered_start;
    if let Some(index) = state.confirmed_column {
        if let Some(line_x) = column_line_x(&layout, 0, index) {
            if line_x < area.x.saturating_add(area.width) {
                for y in top..bottom {
                    buffer.set_string(
                        line_x,
                        y,
                        "│",
                        Style::default().fg(Color::Blue).bg(UI_BACKGROUND),
                    );
                }
            }
        }
    } else if state.confirmed_start || implicit_start {
        if let Some(line_x) = start_line_x(&layout) {
            if line_x < area.x.saturating_add(area.width) {
                for y in top..bottom {
                    buffer.set_string(
                        line_x,
                        y,
                        "│",
                        Style::default().fg(Color::Blue).bg(UI_BACKGROUND),
                    );
                }
            }
        }
    }

    if let Some((row, index)) = state.hovered_column {
        if Some(index) != state.confirmed_column {
            if let Some(line_x) = column_line_x(&layout, row, index) {
                if line_x < area.x.saturating_add(area.width) {
                    for y in top..bottom {
                        buffer.set_string(
                            line_x,
                            y,
                            "│",
                            Style::default().fg(Color::DarkGray).bg(UI_BACKGROUND),
                        );
                    }
                }
            }
        }
    } else if state.hovered_start && !state.confirmed_start {
        if let Some(line_x) = start_line_x(&layout) {
            if line_x < area.x.saturating_add(area.width) {
                for y in top..bottom {
                    buffer.set_string(
                        line_x,
                        y,
                        "│",
                        Style::default().fg(Color::DarkGray).bg(UI_BACKGROUND),
                    );
                }
            }
        }
    }

    if current_qubits >= 2 {
        let max_rows = layout.slots.len();
        let max_cols = layout
            .slots
            .iter()
            .map(|row| row.len())
            .min()
            .unwrap_or(0);
        let drag_virtual = state.dragging.and_then(|drag_state| {
            let row = state.hovered_row?;
            let slot = state.hovered_slot?;
            if state
                .placed
                .get(row)
                .and_then(|row_gates| row_gates.get(slot))
                .and_then(|gate| *gate)
                .is_some()
            {
                return None;
            }
            Some((row, slot, drag_state.gate))
        });
        let gate_at = |row: usize, slot: usize| -> Option<Gate> {
            if let Some((drag_row, drag_slot, drag_gate)) = drag_virtual {
                if row == drag_row && slot == drag_slot {
                    return Some(drag_gate);
                }
            }
            state
                .placed
                .get(row)
                .and_then(|row_gates| row_gates.get(slot))
                .and_then(|gate| *gate)
        };
        for slot in 0..max_cols {
            let mut swap_rows = Vec::new();
            for row in 0..max_rows {
                if gate_at(row, slot) == Some(Gate::Swap) {
                    swap_rows.push(row);
                }
            }
            if swap_rows.len() != 2 {
                continue;
            }
            let row0 = swap_rows[0];
            let row1 = swap_rows[1];
            let rect0 = layout.slots[row0][slot];
            let rect1 = layout.slots[row1][slot];
            let line_x = rect0.x.saturating_add(rect0.width / 2);
            let start_y = rect0.y.saturating_add(rect0.height / 2);
            let end_y = rect1.y.saturating_add(rect1.height / 2);
            let (_, background, _, _) = gate_theme(Gate::Swap);
            let line_style = Style::default().fg(background).bg(background);
            let (top, bottom) = if start_y <= end_y {
                (start_y, end_y)
            } else {
                (end_y, start_y)
            };
            for y in top..=bottom {
                buffer.set_string(line_x, y, "│", line_style);
            }
        }
        for slot in 0..max_cols {
            let mut control_rows = Vec::new();
            let mut target_rows = Vec::new();
            for row in 0..max_rows {
                let gate = gate_at(row, slot);
                match gate {
                    Some(Gate::Control) => control_rows.push(row),
                    Some(Gate::X) => target_rows.push(row),
                    _ => {}
                }
            }
            if control_rows.is_empty() || target_rows.is_empty() {
                continue;
            }
            let line_row = *control_rows
                .first()
                .or_else(|| target_rows.first())
                .unwrap();
            let line_rect = layout.slots[line_row][slot];
            let line_x = line_rect.x.saturating_add(line_rect.width / 2);
            let min_row = control_rows
                .iter()
                .chain(target_rows.iter())
                .copied()
                .min()
                .unwrap_or(line_row);
            let max_row = control_rows
                .iter()
                .chain(target_rows.iter())
                .copied()
                .max()
                .unwrap_or(line_row);
            let min_rect = layout.slots[min_row][slot];
            let max_rect = layout.slots[max_row][slot];
            let start_y = min_rect.y.saturating_add(min_rect.height / 2);
            let end_y = max_rect.y.saturating_add(max_rect.height / 2);
            let (_, background, _, _) = gate_theme(Gate::Control);
            let line_style = Style::default().fg(background).bg(background);
            let (top, bottom) = if start_y <= end_y {
                (start_y, end_y)
            } else {
                (end_y, start_y)
            };
            for y in top..=bottom {
                buffer.set_string(line_x, y, "│", line_style);
            }
        }
    }
    let state_limit = state
        .hovered_column
        .map(|(_, index)| index + 1)
        .or(state.confirmed_column.map(|index| index + 1))
        .or(Some(0));
    if !state.cached_full_valid {
        let full_sim = apply_gates_to_zero_limit(&state.placed, None, Some(&state.phase_values));
        state.cached_full_measurements = full_sim.measurements;
        state.cached_full_valid = true;
    }
    if !state.cache_valid || state.cached_limit != state_limit {
        let simulation =
            apply_gates_to_zero_limit(&state.placed, state_limit, Some(&state.phase_values));
        state.cached_state = simulation.state;
        state.cached_limit = state_limit;
        state.cache_valid = true;
    }
    for (row, row_slots) in layout.slots.iter().enumerate() {
        for (slot, rect) in row_slots.iter().enumerate() {
            if let Some(Some(gate)) = state
                .placed
                .get(row)
                .and_then(|row_gates| row_gates.get(slot))
            {
                let (phase_label, phase_edit_active) = if matches!(
                    *gate,
                    Gate::Phase | Gate::Rx | Gate::Ry | Gate::Rz
                ) {
                    let label = if let Some(edit) = state.phase_edit.as_ref() {
                        if edit.row == row && edit.slot == slot {
                            edit.input.clone()
                        } else {
                            state
                                .phase_values
                                .get(row)
                                .and_then(|row_values| row_values.get(slot))
                                .and_then(|value| value.as_ref())
                                .map(|value| value.label.clone())
                                .unwrap_or_else(|| default_phase_value().label)
                        }
                    } else {
                        state
                            .phase_values
                            .get(row)
                            .and_then(|row_values| row_values.get(slot))
                            .and_then(|value| value.as_ref())
                            .map(|value| value.label.clone())
                            .unwrap_or_else(|| default_phase_value().label)
                    };
                    let is_active = state
                        .phase_edit
                        .as_ref()
                        .is_some_and(|edit| edit.row == row && edit.slot == slot);
                    (Some(label), is_active)
                } else {
                    (None, false)
                };
                let measure_value = state
                    .cached_full_measurements
                    .get(row)
                    .and_then(|row_values| row_values.get(slot))
                    .and_then(|value| *value);
                draw_gate_box(
                    &mut buffer,
                    *rect,
                    *gate,
                    measure_value,
                    phase_label.as_deref(),
                    phase_edit_active,
                );
            }
        }
    }
    if state.dragging.is_some() {
        // No placeholder rendering; snapping is handled by the drag visual.
    }
    let _ = debug_line;
    let lines = format_state_histogram(
        &state.cached_state,
        current_qubits,
        regions.state.height,
        regions.state.width,
    );
    let text = Text::from(lines);
    let paragraph = Paragraph::new(text).style(Style::default().bg(UI_BACKGROUND));
    paragraph.render(regions.state, &mut buffer);
    if let Some(drag) = drag {
        let mut rect = Rect {
            x: drag.x,
            y: drag.y,
            width: GATE_BOX_WIDTH,
            height: GATE_DRAW_HEIGHT,
        };
        if state.dragging.is_some() {
            if let Some((row, index)) = state.hovered_insert {
                if let Some(insert_rect) = insertion_snap_rect(&layout, row, index) {
                    rect = insert_rect;
                }
            } else if let (Some(row), Some(slot)) = (state.hovered_row, state.hovered_slot) {
                if state.placed[row].get(slot).and_then(|value| *value).is_none() {
                    if let Some(slot_rect) = layout
                        .slots
                        .get(row)
                        .and_then(|row_slots| row_slots.get(slot))
                    {
                        rect = *slot_rect;
                    }
                }
            }
        }
        if rect.x >= area.x
            && rect.y >= area.y
            && rect.x < area.x.saturating_add(area.width)
            && rect.y < area.y.saturating_add(area.height)
        {
            draw_gate_box(&mut buffer, rect, drag.gate, None, None, false);
        }
    }
    buffer
}

fn format_state_histogram(
    amplitudes: &[Complex],
    qubits: usize,
    max_lines: u16,
    width: u16,
) -> Vec<Line<'static>> {
    let total = amplitudes.len();
    let max_lines = max_lines as usize;
    let width = width as usize;
    if max_lines == 0 || width == 0 || total == 0 {
        return Vec::new();
    }
    const COLUMN_GAP: usize = 2;
    let label_width = format!("|{:0width$b}>", 0, width = qubits).len();
    let prob_width = format!("{:.3}", 0.0_f64).len();
    let min_column_width = label_width + 1 + prob_width;
    let max_columns_by_width = if width >= min_column_width {
        ((width + COLUMN_GAP) / (min_column_width + COLUMN_GAP)).max(1)
    } else {
        1
    };
    let columns_needed = (total + max_lines - 1) / max_lines;
    let columns = columns_needed.min(max_columns_by_width).max(1);
    let available_width = width.saturating_sub(COLUMN_GAP.saturating_mul(columns.saturating_sub(1)));
    let column_width = (available_width / columns).max(1);
    let mut visible_states = total.min(columns * max_lines);
    let truncated = visible_states < total;
    if truncated && visible_states > 0 {
        visible_states = visible_states.saturating_sub(1);
    }
    let mut entries = Vec::new();
    for (index, amp) in amplitudes.iter().take(visible_states).enumerate() {
        entries.push(build_histogram_line(amp, index, qubits, column_width));
    }
    if truncated {
        entries.push(trim_line_to_width(Line::from("..."), column_width));
    }
    if entries.is_empty() {
        return Vec::new();
    }
    let rows = max_lines.min(entries.len());
    let mut lines = Vec::with_capacity(rows);
    for row in 0..rows {
        let mut row_spans = Vec::new();
        for column in 0..columns {
            let entry_index = column * max_lines + row;
            let mut spans = if entry_index < entries.len() {
                entries[entry_index].spans.clone()
            } else {
                Vec::new()
            };
            let width = spans_width(&spans);
            if width < column_width {
                spans.push(Span::raw(" ".repeat(column_width - width)));
            }
            row_spans.extend(spans);
            if column + 1 < columns {
                row_spans.push(Span::raw(" ".repeat(COLUMN_GAP)));
            }
        }
        lines.push(Line::from(row_spans));
    }
    lines
}

fn build_histogram_line(
    amp: &Complex,
    index: usize,
    qubits: usize,
    width: usize,
) -> Line<'static> {
    let prob = amp.re * amp.re + amp.im * amp.im;
    let label = format!("|{:0width$b}>", index, width = qubits);
    let prob_text = format!("{:.3}", prob);
    let base_len = label.len() + 1;
    let bar_capacity = width.saturating_sub(base_len);
    let label_style = Style::default().fg(Color::DarkGray);
    let mut spans = Vec::new();
    spans.push(Span::styled(label, label_style));
    spans.push(Span::raw(" "));
    if bar_capacity == 0 {
        spans.push(Span::raw(prob_text));
    } else {
        let bar_spans = build_bar_spans(prob, bar_capacity, &prob_text);
        spans.extend(bar_spans);
    }
    trim_line_to_width(Line::from(spans), width)
}

fn spans_width(spans: &[Span<'_>]) -> usize {
    spans
        .iter()
        .map(|span| span.content.chars().count())
        .sum::<usize>()
}

fn trim_line_to_width(line: Line<'static>, width: usize) -> Line<'static> {
    let mut remaining = width;
    let mut spans = Vec::new();
    for span in line.spans {
        if remaining == 0 {
            break;
        }
        let span_width = span.content.chars().count();
        if span_width <= remaining {
            remaining -= span_width;
            spans.push(span);
        } else {
            let truncated: String = span.content.chars().take(remaining).collect();
            spans.push(Span::styled(truncated, span.style));
            remaining = 0;
        }
    }
    Line::from(spans)
}

fn build_bar_spans(prob: f64, width: usize, label: &str) -> Vec<Span<'static>> {
    if width == 0 {
        return Vec::new();
    }
    let ratio = prob.clamp(0.0, 1.0);
    let units = ratio * width as f64;
    let full = units.floor() as usize;
    let frac = units - full as f64;
    let mut cells = vec![' '; width];
    let mut filled = vec![false; width];
    for i in 0..full.min(width) {
        cells[i] = '█';
        filled[i] = true;
    }
    if full < width {
        let partial = partial_block(frac);
        if partial != ' ' {
            cells[full] = partial;
            filled[full] = true;
        }
    }
    let label_chars: Vec<char> = label.chars().collect();
    if label_chars.len() <= width {
        for (offset, ch) in label_chars.into_iter().enumerate() {
            cells[offset] = ch;
        }
    }
    let normal = Style::default().fg(Color::White).bg(UI_BACKGROUND);
    let inverted = Style::default().fg(UI_BACKGROUND).bg(Color::White);
    let mut spans = Vec::new();
    let mut current = String::new();
    let mut current_style = None;
    for (idx, ch) in cells.iter().enumerate() {
        let is_label = idx < label.len();
        let style = if is_label && filled[idx] {
            inverted
        } else {
            normal
        };
        if current_style.is_none() {
            current_style = Some(style);
            current.push(*ch);
            continue;
        }
        if current_style == Some(style) {
            current.push(*ch);
        } else {
            spans.push(Span::styled(current, current_style.unwrap()));
            current = String::new();
            current.push(*ch);
            current_style = Some(style);
        }
    }
    if let Some(style) = current_style {
        spans.push(Span::styled(current, style));
    }
    spans
}

fn partial_block(fraction: f64) -> char {
    match fraction {
        f if f >= 7.0 / 8.0 => '▉',
        f if f >= 6.0 / 8.0 => '▊',
        f if f >= 5.0 / 8.0 => '▋',
        f if f >= 4.0 / 8.0 => '▌',
        f if f >= 3.0 / 8.0 => '▍',
        f if f >= 2.0 / 8.0 => '▎',
        f if f >= 1.0 / 8.0 => '▏',
        _ => ' ',
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_to_string(line: &Line<'static>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }

    #[test]
    fn histogram_uses_multiple_columns_when_height_is_limited() {
        let amplitudes = vec![
            Complex { re: 1.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
        ];
        let lines = format_state_histogram(&amplitudes, 3, 4, 40);
        assert_eq!(lines.len(), 4);
        let row0 = line_to_string(&lines[0]);
        let row1 = line_to_string(&lines[1]);
        let row2 = line_to_string(&lines[2]);
        let row3 = line_to_string(&lines[3]);
        assert!(row0.contains("|000>"));
        assert!(row0.contains("|100>"));
        assert!(row1.contains("|001>"));
        assert!(row1.contains("|101>"));
        assert!(row2.contains("|010>"));
        assert!(row2.contains("|110>"));
        assert!(row3.contains("|011>"));
        assert!(row3.contains("|111>"));
    }

    #[test]
    fn histogram_truncates_with_ellipsis_when_columns_do_not_fit() {
        let amplitudes = vec![
            Complex { re: 1.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
        ];
        let lines = format_state_histogram(&amplitudes, 3, 3, 16);
        assert_eq!(lines.len(), 3);
        let last = line_to_string(&lines[2]).trim_end().to_string();
        assert_eq!(last, "...");
    }
}
