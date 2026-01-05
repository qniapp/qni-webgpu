use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Text;
use ratatui::widgets::{Paragraph, Widget};

use crate::layout::{
    circuit_layout, column_line_x, insertion_snap_rect, layout_regions, palette_items, start_line_x,
};
use crate::model::{build_state_line_with_limit, ensure_slots, qubit_count, AppState, Gate};
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
        Gate::H => (255, 220, 100),
        Gate::X => (220, 80, 80),
        Gate::Y => (180, 120, 200),
        Gate::Z => (90, 120, 220),
        Gate::SqrtX => (220, 80, 80),
        Gate::S => (110, 200, 110),
        Gate::Sdg => (110, 200, 110),
        Gate::T => (110, 200, 200),
        Gate::Tdg => (110, 200, 200),
        Gate::Swap => (140, 170, 230),
    };
    let background = Color::Rgb(r, g, b);
    let highlight = brighten(r, g, b, 60);
    let shadow = darken(r, g, b, 60);
    let text = Color::Black;
    (text, background, highlight, shadow)
}

fn brighten(r: u8, g: u8, b: u8, delta: u8) -> Color {
    Color::Rgb(r.saturating_add(delta), g.saturating_add(delta), b.saturating_add(delta))
}

fn darken(r: u8, g: u8, b: u8, delta: u8) -> Color {
    Color::Rgb(r.saturating_sub(delta), g.saturating_sub(delta), b.saturating_sub(delta))
}

pub(crate) fn draw_gate_box(buffer: &mut Buffer, rect: Rect, gate: Gate) {
    if rect.width < GATE_BOX_WIDTH || rect.height < GATE_DRAW_HEIGHT {
        return;
    }
    let (text, background, highlight, shadow) = gate_theme(gate);
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
    let gate_rect = Rect {
        x: rect.x,
        y: rect.y + SHADOW_OUTSET,
        width: rect.width,
        height: GATE_BOX_HEIGHT,
    };
    if gate == Gate::Swap {
        let fill = " ".repeat(gate_rect.width as usize);
        let mid_y = gate_rect.y.saturating_add(gate_rect.height / 2);
        buffer.set_string(gate_rect.x, mid_y, &fill, base_style);
        if gate_rect.width > 2 {
            let gap = " ".repeat(gate_rect.width.saturating_sub(2) as usize);
            let gap_style = Style::default().fg(UI_BACKGROUND).bg(UI_BACKGROUND);
            buffer.set_string(gate_rect.x + 1, gate_rect.y, &gap, gap_style);
            buffer.set_string(
                gate_rect.x + 1,
                gate_rect.y + gate_rect.height.saturating_sub(1),
                &gap,
                gap_style,
            );
        }
        let left_x = gate_rect.x;
        let right_x = gate_rect.x.saturating_add(gate_rect.width.saturating_sub(1));
        let top_y = gate_rect.y;
        let bottom_y = gate_rect.y.saturating_add(gate_rect.height.saturating_sub(1));
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
        draw_gate_box(&mut buffer, item.rect, item.gate);
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

    for (row, row_slots) in layout.slots.iter().enumerate() {
        for (slot, rect) in row_slots.iter().enumerate() {
            if let Some(Some(gate)) = state
                .placed
                .get(row)
                .and_then(|row_gates| row_gates.get(slot))
            {
                draw_gate_box(&mut buffer, *rect, *gate);
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
            let line_style = Style::default()
                .fg(Color::LightBlue)
                .bg(Color::LightBlue);
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
    if state.dragging.is_some() {
        // No placeholder rendering; snapping is handled by the drag visual.
    }
    if let Some(debug) = debug_line {
        if !debug.trim().is_empty() {
            let text = Text::from(format!("Debug: {}", debug));
            let paragraph = Paragraph::new(text).style(Style::default().bg(UI_BACKGROUND));
            let debug_area = Rect {
                x: area.x,
                y: regions.state.y.saturating_sub(1),
                width: area.width,
                height: 1,
            };
            paragraph.render(debug_area, &mut buffer);
        }
    }
    let state_limit = state
        .hovered_column
        .map(|(_, index)| index + 1)
        .or(state.confirmed_column.map(|index| index + 1))
        .or(Some(0));
    let state_line = build_state_line_with_limit(&state.placed, state_limit);
    let text = Text::from(state_line);
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
            draw_gate_box(&mut buffer, rect, drag.gate);
        }
    }
    buffer
}
