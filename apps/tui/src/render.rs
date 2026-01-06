use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::widgets::canvas::{Canvas, Circle, Line, Points};
use ratatui::widgets::Widget;

use crate::layout::{
    circuit_layout, column_line_x, insertion_snap_rect, layout_regions, palette_items,
    start_line_x, CircuitLayout,
};
use crate::model::{
    apply_gates_to_zero_limit, default_phase_value, ensure_slots, qubit_count, AppState, Complex,
    Gate, QuitChoice,
};
use crate::{
    GATE_BOX_HEIGHT, GATE_BOX_WIDTH, GATE_DRAW_HEIGHT, PALETTE_GAP, PALETTE_LABEL, SHADOW_OUTSET,
    UI_BACKGROUND,
};

const DRAG_GATE_COLOR: Color = Color::Rgb(34, 211, 238);
const MODAL_BG: Color = Color::DarkGray;
const MODAL_BORDER: Color = Color::Gray;
const STATE_CIRCLE_OUTLINE: Color = Color::White;
const STATE_CIRCLE_FILL: Color = Color::LightCyan;
const STATE_CIRCLE_PHASE: Color = Color::Yellow;
const STATE_CIRCLE_ZERO: Color = Color::DarkGray;

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

fn gate_theme_with_override(
    gate: Gate,
    override_background: Option<Color>,
) -> (Color, Color, Color, Color) {
    let Some(background) = override_background else {
        return gate_theme(gate);
    };
    let text = Color::White;
    match background {
        Color::Rgb(r, g, b) => (
            text,
            background,
            brighten(r, g, b, 60),
            darken(r, g, b, 60),
        ),
        _ => (text, background, background, background),
    }
}

fn brighten(r: u8, g: u8, b: u8, delta: u8) -> Color {
    Color::Rgb(
        r.saturating_add(delta),
        g.saturating_add(delta),
        b.saturating_add(delta),
    )
}

fn darken(r: u8, g: u8, b: u8, delta: u8) -> Color {
    Color::Rgb(
        r.saturating_sub(delta),
        g.saturating_sub(delta),
        b.saturating_sub(delta),
    )
}

fn draw_gate_outline(buffer: &mut Buffer, rect: Rect, highlight: Color, shadow: Color) {
    if rect.width < 2 || rect.height < 2 {
        return;
    }
    let top = "▔".repeat(rect.width as usize);
    let bottom = "▁".repeat(rect.width as usize);
    let highlight_style = Style::default().fg(highlight).bg(UI_BACKGROUND);
    let shadow_style = Style::default().fg(shadow).bg(UI_BACKGROUND);
    buffer.set_string(rect.x, rect.y, &top, highlight_style);
    buffer.set_string(
        rect.x,
        rect.y.saturating_add(rect.height.saturating_sub(1)),
        &bottom,
        shadow_style,
    );
    if rect.height > 2 {
        for offset in 1..rect.height.saturating_sub(1) {
            let y = rect.y.saturating_add(offset);
            buffer.set_string(rect.x, y, "▏", highlight_style);
            buffer.set_string(
                rect.x.saturating_add(rect.width.saturating_sub(1)),
                y,
                "▕",
                shadow_style,
            );
        }
    }
}

fn draw_quit_modal(buffer: &mut Buffer, area: Rect, choice: QuitChoice) {
    if area.width < 20 || area.height < 7 {
        return;
    }
    let width = 28.min(area.width.saturating_sub(2)).max(20);
    let height = 7;
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let rect = Rect {
        x,
        y,
        width,
        height,
    };
    let fill = " ".repeat(rect.width as usize);
    let fill_style = Style::default().fg(Color::White).bg(MODAL_BG);
    for offset in 0..rect.height {
        buffer.set_string(rect.x, rect.y + offset, &fill, fill_style);
    }
    let border_style = Style::default().fg(MODAL_BORDER).bg(MODAL_BG);
    let top = format!("┌{}┐", "─".repeat(rect.width.saturating_sub(2) as usize));
    let bottom = format!("└{}┘", "─".repeat(rect.width.saturating_sub(2) as usize));
    buffer.set_string(rect.x, rect.y, &top, border_style);
    buffer.set_string(
        rect.x,
        rect.y + rect.height.saturating_sub(1),
        &bottom,
        border_style,
    );
    for offset in 1..rect.height.saturating_sub(1) {
        let y = rect.y + offset;
        buffer.set_string(rect.x, y, "│", border_style);
        buffer.set_string(
            rect.x + rect.width.saturating_sub(1),
            y,
            "│",
            border_style,
        );
    }
    let title = "Quit?";
    let title_x = rect.x + (rect.width.saturating_sub(title.len() as u16)) / 2;
    buffer.set_string(
        title_x,
        rect.y + 1,
        title,
        Style::default().fg(Color::White).bg(MODAL_BG).add_modifier(Modifier::BOLD),
    );
    let yes_label = "[ Yes ]";
    let no_label = "[ No ]";
    let buttons_width = yes_label.len() + 2 + no_label.len();
    let start_x = rect.x + (rect.width.saturating_sub(buttons_width as u16)) / 2;
    let yes_style = if choice == QuitChoice::Yes {
        Style::default()
            .fg(Color::Black)
            .bg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White).bg(MODAL_BG)
    };
    let no_style = if choice == QuitChoice::No {
        Style::default()
            .fg(Color::Black)
            .bg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White).bg(MODAL_BG)
    };
    let buttons_y = rect.y + rect.height.saturating_sub(3);
    buffer.set_string(start_x, buttons_y, yes_label, yes_style);
    buffer.set_string(
        start_x + yes_label.len() as u16 + 2,
        buttons_y,
        no_label,
        no_style,
    );
}

#[derive(Clone)]
struct DeferredGate {
    rect: Rect,
    gate: Gate,
    phase_label: Option<String>,
    phase_edit_active: bool,
    measure_value: Option<u8>,
}

fn draw_state_circles(buffer: &mut Buffer, area: Rect, amplitudes: &[Complex]) {
    if area.width == 0 || area.height == 0 || amplitudes.is_empty() {
        return;
    }
    let qubits = amplitude_qubits(amplitudes.len());
    let min_cell_w = 4.0_f64;
    let min_cell_h = 3.0_f64;
    let max_cols = ((area.width as f64) / min_cell_w).floor() as usize;
    let max_rows = ((area.height as f64) / min_cell_h).floor() as usize;
    if max_cols == 0 || max_rows == 0 {
        return;
    }
    let total = amplitudes.len();
    let columns_needed = (total + max_rows - 1) / max_rows;
    let columns = columns_needed.min(max_cols).max(1);
    let rows = ((total + columns - 1) / columns).min(max_rows).max(1);
    let visible = rows * columns;
    let cell_w = area.width as f64 / columns as f64;
    let cell_h = area.height as f64 / rows as f64;
    let size_boost = match qubits {
        0 | 1 => 1.15,
        2 => 1.1,
        3 => 1.05,
        _ => 1.0,
    };
    let base_radius = ((cell_w.min(cell_h) / 2.0) + 0.3) * size_boost;
    if base_radius <= 0.1 {
        return;
    }
    let x_bounds = [0.0, area.width as f64];
    let y_bounds = [0.0, area.height as f64];
    let canvas = Canvas::default()
        .x_bounds(x_bounds)
        .y_bounds(y_bounds)
        .marker(Marker::HalfBlock)
        .paint(|ctx| {
            for index in 0..visible {
                let state_index = display_index_to_state_index(index, qubits);
                if state_index >= amplitudes.len() {
                    continue;
                }
                let amp = amplitudes[state_index];
                let row = index / columns;
                let col = index % columns;
                let center_x = col as f64 * cell_w + cell_w / 2.0;
                let center_y = (rows as f64 - 1.0 - row as f64) * cell_h + cell_h / 2.0;
                let prob = amp.re * amp.re + amp.im * amp.im;
                let is_zero = prob <= 1e-6;
                let outline_color = if is_zero {
                    STATE_CIRCLE_ZERO
                } else {
                    STATE_CIRCLE_OUTLINE
                };
                ctx.draw(&Circle {
                    x: center_x,
                    y: center_y,
                    radius: base_radius,
                    color: outline_color,
                });
                let fill_radius = (prob.clamp(0.0, 1.0)).sqrt() * base_radius;
                if fill_radius > 0.1 {
                    let mut points = Vec::new();
                    let step = 0.35;
                    let mut y = -fill_radius;
                    while y <= fill_radius {
                        let mut x = -fill_radius;
                        while x <= fill_radius {
                            if x * x + y * y <= fill_radius * fill_radius {
                                points.push((center_x + x, center_y + y));
                            }
                            x += step;
                        }
                        y += step;
                    }
                    if !points.is_empty() {
                        ctx.draw(&Points {
                            coords: &points,
                            color: STATE_CIRCLE_FILL,
                        });
                    }
                }
                if !is_zero {
                    let phase = amp.im.atan2(amp.re);
                    let angle = phase + std::f64::consts::FRAC_PI_2;
                    let phase_radius = base_radius * 0.75;
                    let end_x = center_x + phase_radius * angle.cos();
                    let end_y = center_y + phase_radius * angle.sin();
                    ctx.draw(&Line {
                        x1: center_x,
                        y1: center_y,
                        x2: end_x,
                        y2: end_y,
                        color: STATE_CIRCLE_PHASE,
                    });
                    let phase_tip = [(end_x, end_y)];
                    ctx.draw(&Points {
                        coords: &phase_tip,
                        color: STATE_CIRCLE_PHASE,
                    });
                }
            }
        });
    canvas.render(area, buffer);
}

fn amplitude_qubits(len: usize) -> usize {
    let mut size = len.max(1);
    let mut qubits = 0;
    while size > 1 {
        size >>= 1;
        qubits += 1;
    }
    qubits
}

fn display_index_to_state_index(display_index: usize, qubits: usize) -> usize {
    let mut value = display_index;
    let mut reversed = 0usize;
    for _ in 0..qubits {
        reversed = (reversed << 1) | (value & 1);
        value >>= 1;
    }
    reversed
}

pub(crate) fn draw_gate_box(
    buffer: &mut Buffer,
    rect: Rect,
    gate: Gate,
    measure_value: Option<u8>,
    phase_label: Option<&str>,
    phase_edit_active: bool,
    override_background: Option<Color>,
    outline: bool,
) {
    if rect.width < GATE_BOX_WIDTH || rect.height < GATE_DRAW_HEIGHT {
        return;
    }
    let (text, background, highlight, shadow) =
        gate_theme_with_override(gate, override_background);
    if gate == Gate::Control {
        if outline {
            draw_gate_outline(buffer, rect, highlight, shadow);
        }
        let style = Style::default()
            .fg(background)
            .bg(UI_BACKGROUND)
            .add_modifier(Modifier::BOLD);
        let mid_y = rect.y.saturating_add(rect.height / 2);
        let mid_x = rect.x.saturating_add(rect.width / 2);
        buffer.set_string(mid_x, mid_y, "■", style);
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
    if matches!(gate, Gate::Phase | Gate::Rx | Gate::Ry | Gate::Rz) && rect.width >= 3 && rect.y > 0
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
        let top_y = swap_rect.y;
        let bottom_y = swap_rect
            .y
            .saturating_add(swap_rect.height.saturating_sub(1));
        let tip_style = Style::default().fg(background).bg(UI_BACKGROUND);
        if swap_rect.width >= 4 {
            buffer.set_string(swap_rect.x, top_y, "▐ ", tip_style);
            buffer.set_string(
                swap_rect.x.saturating_add(swap_rect.width - 2),
                top_y,
                " ▌",
                tip_style,
            );
            buffer.set_string(swap_rect.x, bottom_y, "▐ ", tip_style);
            buffer.set_string(
                swap_rect.x.saturating_add(swap_rect.width - 2),
                bottom_y,
                " ▌",
                tip_style,
            );
        } else {
            buffer.set_string(swap_rect.x, top_y, "▐", tip_style);
            buffer.set_string(
                swap_rect
                    .x
                    .saturating_add(swap_rect.width.saturating_sub(1)),
                top_y,
                "▌",
                tip_style,
            );
            buffer.set_string(swap_rect.x, bottom_y, "▐", tip_style);
            buffer.set_string(
                swap_rect
                    .x
                    .saturating_add(swap_rect.width.saturating_sub(1)),
                bottom_y,
                "▌",
                tip_style,
            );
        }
        let left_x = if swap_rect.width > 2 {
            swap_rect.x + 1
        } else {
            swap_rect.x
        };
        let right_x = if swap_rect.width > 2 {
            swap_rect
                .x
                .saturating_add(swap_rect.width.saturating_sub(2))
        } else {
            swap_rect
                .x
                .saturating_add(swap_rect.width.saturating_sub(1))
        };
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
            buffer.set_string(gate_rect.x + gate_rect.width - 1, gate_rect.y, "◥", corner);
            buffer.set_string(gate_rect.x, gate_rect.y + gate_rect.height - 1, "◣", corner);
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
        draw_gate_box(&mut buffer, item.rect, item.gate, None, None, false, None, false);
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
        let max_cols = layout.slots.iter().map(|row| row.len()).min().unwrap_or(0);
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
            let line_style = Style::default().fg(background).bg(UI_BACKGROUND);
            let half_style = Style::default().fg(background).bg(UI_BACKGROUND);
            let cuts =
                build_line_cuts(&layout, slot, max_rows, &gate_at, |gate| gate == Gate::Swap);
            let (top, bottom) = if start_y <= end_y {
                (start_y, end_y)
            } else {
                (end_y, start_y)
            };
            for y in top..=bottom {
                if let Some(ch) = line_cut_char(y, &cuts) {
                    buffer.set_string(line_x, y, ch, half_style);
                    continue;
                }
                if is_line_skip(y, &cuts) {
                    continue;
                }
                buffer.set_string(line_x, y, "┃", line_style);
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
            let line_style = Style::default().fg(background).bg(UI_BACKGROUND);
            let half_style = Style::default().fg(background).bg(UI_BACKGROUND);
            let cuts = build_line_cuts(&layout, slot, max_rows, &gate_at, |gate| {
                matches!(gate, Gate::Control | Gate::X)
            });
            let (top, bottom) = if start_y <= end_y {
                (start_y, end_y)
            } else {
                (end_y, start_y)
            };
            for y in top..=bottom {
                if let Some(ch) = line_cut_char(y, &cuts) {
                    buffer.set_string(line_x, y, ch, half_style);
                    continue;
                }
                if is_line_skip(y, &cuts) {
                    continue;
                }
                buffer.set_string(line_x, y, "┃", line_style);
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
    let mut deferred_gate: Option<DeferredGate> = None;
    let defer_target = state
        .dragging
        .is_some()
        .then_some(state.hovered_insert)
        .flatten();
    for (row, row_slots) in layout.slots.iter().enumerate() {
        for (slot, rect) in row_slots.iter().enumerate() {
            if let Some(Some(gate)) = state
                .placed
                .get(row)
                .and_then(|row_gates| row_gates.get(slot))
            {
                let (phase_label, phase_edit_active) =
                    if matches!(*gate, Gate::Phase | Gate::Rx | Gate::Ry | Gate::Rz) {
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
                        if is_active {
                            let max_chars = rect.width as usize;
                            let mut chars: Vec<char> = label.chars().collect();
                            if max_chars > 0 {
                                if chars.len() < max_chars {
                                    chars.push('▌');
                                } else {
                                    chars[max_chars.saturating_sub(1)] = '▌';
                                }
                            }
                            let label_with_cursor: String = chars.into_iter().collect();
                            (Some(label_with_cursor), is_active)
                        } else {
                            (Some(label), is_active)
                        }
                    } else {
                        (None, false)
                    };
                let measure_value = state
                    .cached_full_measurements
                    .get(row)
                    .and_then(|row_values| row_values.get(slot))
                    .and_then(|value| *value);
                if let Some((target_row, target_index)) = defer_target {
                    if row == target_row && slot == target_index {
                        let left_gate = target_index
                            .checked_sub(1)
                            .and_then(|index| state.placed.get(row)?.get(index))
                            .and_then(|gate| *gate);
                        if left_gate.is_some() {
                            deferred_gate = Some(DeferredGate {
                                rect: *rect,
                                gate: *gate,
                                phase_label,
                                phase_edit_active,
                                measure_value,
                            });
                            continue;
                        }
                    }
                }
                draw_gate_box(
                    &mut buffer,
                    *rect,
                    *gate,
                    measure_value,
                    phase_label.as_deref(),
                    phase_edit_active,
                    None,
                    false,
                );
            }
        }
    }
    if state.dragging.is_some() {
        // No placeholder rendering; snapping is handled by the drag visual.
    }
    let _ = debug_line;
    draw_state_circles(&mut buffer, regions.state, &state.cached_state);
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
                if state.placed[row]
                    .get(slot)
                    .and_then(|value| *value)
                    .is_none()
                {
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
            draw_gate_box(
                &mut buffer,
                rect,
                drag.gate,
                None,
                None,
                false,
                Some(DRAG_GATE_COLOR),
                drag.gate == Gate::Control,
            );
        }
    }
    if let Some(deferred_gate) = deferred_gate {
        draw_gate_box(
            &mut buffer,
            deferred_gate.rect,
            deferred_gate.gate,
            deferred_gate.measure_value,
            deferred_gate.phase_label.as_deref(),
            deferred_gate.phase_edit_active,
            None,
            false,
        );
    }
    if state.quit_confirm {
        draw_quit_modal(&mut buffer, area, state.quit_choice);
    }
    buffer
}


#[derive(Clone, Copy)]
struct LineCut {
    skip_start: u16,
    skip_end: u16,
    upper_half_y: Option<u16>,
    lower_half_y: Option<u16>,
}

fn build_line_cuts<F, G>(
    layout: &CircuitLayout,
    slot: usize,
    max_rows: usize,
    gate_at: &F,
    keep_gate: G,
) -> Vec<LineCut>
where
    F: Fn(usize, usize) -> Option<Gate>,
    G: Fn(Gate) -> bool,
{
    let mut cuts = Vec::new();
    for row in 0..max_rows {
        if let Some(gate) = gate_at(row, slot) {
            if keep_gate(gate) {
                continue;
            }
            if let Some(rect) = layout
                .slots
                .get(row)
                .and_then(|row_slots| row_slots.get(slot))
            {
                let skip_start = rect.y;
                let skip_end = rect.y.saturating_add(rect.height.saturating_sub(1));
                cuts.push(LineCut {
                    skip_start,
                    skip_end,
                    upper_half_y: rect.y.checked_sub(1),
                    lower_half_y: rect.y.checked_add(rect.height),
                });
            }
        }
    }
    cuts
}

fn line_cut_char(y: u16, cuts: &[LineCut]) -> Option<&'static str> {
    for cut in cuts {
        if cut.upper_half_y == Some(y) || cut.lower_half_y == Some(y) {
            return Some("┇");
        }
    }
    None
}

fn is_line_skip(y: u16, cuts: &[LineCut]) -> bool {
    cuts.iter()
        .any(|cut| y >= cut.skip_start && y <= cut.skip_end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_index_to_state_index_reverses_bits() {
        assert_eq!(display_index_to_state_index(0, 2), 0);
        assert_eq!(display_index_to_state_index(1, 2), 2);
        assert_eq!(display_index_to_state_index(2, 2), 1);
        assert_eq!(display_index_to_state_index(3, 2), 3);
    }
}
