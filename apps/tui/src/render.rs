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
        for slot in 0..max_cols {
            let mut swap_rows = Vec::new();
            for row in 0..max_rows {
                let is_swap = state
                    .placed
                    .get(row)
                    .and_then(|row_gates| row_gates.get(slot))
                    .and_then(|gate| *gate)
                    == Some(Gate::Swap);
                if is_swap {
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
            let mut control_row = None;
            let mut target_row = None;
            for row in 0..max_rows {
                let gate = state
                    .placed
                    .get(row)
                    .and_then(|row_gates| row_gates.get(slot))
                    .and_then(|gate| *gate);
                match gate {
                    Some(Gate::Control) => {
                        if control_row.is_none() {
                            control_row = Some(row);
                        }
                    }
                    Some(Gate::X) => {
                        if target_row.is_none() {
                            target_row = Some(row);
                        }
                    }
                    _ => {}
                }
            }
            let (control_row, target_row) = match (control_row, target_row) {
                (Some(control), Some(target)) if control != target => (control, target),
                _ => continue,
            };
            let control_rect = layout.slots[control_row][slot];
            let target_rect = layout.slots[target_row][slot];
            let line_x = control_rect.x.saturating_add(control_rect.width / 2);
            let start_y = control_rect.y.saturating_add(control_rect.height / 2);
            let end_y = target_rect.y.saturating_add(target_rect.height / 2);
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
    if max_lines == 0 || width == 0 {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let visible_states = if max_lines >= total {
        total
    } else {
        max_lines.saturating_sub(1)
    };
    for (index, amp) in amplitudes.iter().take(visible_states).enumerate() {
        let prob = amp.re * amp.re + amp.im * amp.im;
        let label = format!("|{:0width$b}>", index, width = qubits);
        let prob_text = format!("{:.3}", prob);
        let base_len = label.len() + 1 + prob_text.len() + 1;
        let bar_capacity = (width as usize).saturating_sub(base_len);
        let bar_len = ((prob * bar_capacity as f64).round() as usize).min(bar_capacity);
        let bar = if bar_capacity > 0 {
            "█".repeat(bar_len)
        } else {
            String::new()
        };
        let label_style = Style::default().fg(Color::DarkGray);
        let mut spans = Vec::new();
        spans.push(Span::styled(label, label_style));
        spans.push(Span::raw(" "));
        spans.push(Span::raw(prob_text));
        if bar_capacity > 0 {
            spans.push(Span::raw(" "));
            spans.push(Span::raw(bar));
        }
        lines.push(Line::from(spans));
    }
    if max_lines < total {
        lines.push(Line::from("..."));
    }
    lines
}
