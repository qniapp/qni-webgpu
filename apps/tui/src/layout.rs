use ratatui::layout::Rect;

use crate::model::Gate;
use crate::{
    GATE_BOX_WIDTH, GATE_DRAW_HEIGHT, GATE_GAP, MIN_QUBIT_COUNT, PALETTE_GAP, ROW_GAP,
    SEPARATOR_TO_CIRCUIT_GAP, SHADOW_OUTSET, SLOT_GAP,
};

const WIRE_PREFIX: &str = "q0: ";
const SLOT_PADDING_LEFT: u16 = 3;
const PALETTE_GATES: [Gate; 16] = [
    Gate::H,
    Gate::X,
    Gate::Y,
    Gate::Z,
    Gate::SqrtX,
    Gate::S,
    Gate::Sdg,
    Gate::T,
    Gate::Tdg,
    Gate::Phase,
    Gate::Rx,
    Gate::Ry,
    Gate::Rz,
    Gate::Swap,
    Gate::Control,
    Gate::Measure,
];

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PaletteItem {
    pub gate: Gate,
    pub rect: Rect,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Regions {
    pub palette: Rect,
    pub circuits: Vec<Rect>,
    pub state: Rect,
    pub state_popup: Rect,
    pub state_circles: Rect,
}

#[derive(Debug)]
pub struct CircuitLayout {
    pub wire_start_x: u16,
    pub wire_width: u16,
    pub wire_rows: Vec<u16>,
    pub slots: Vec<Vec<Rect>>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct StateCircleLayout {
    pub columns: usize,
    pub rows: usize,
    pub cell_w: f64,
    pub cell_h: f64,
    pub visible: usize,
    pub origin_x: f64,
    pub origin_y: f64,
    pub step_x: f64,
    pub step_y: f64,
}

pub fn layout_regions(area: Rect, qubit_count: usize) -> Regions {
    let palette_height = palette_height(area.width);
    let palette_bottom = area.y.saturating_add(palette_height);
    let separator_y = palette_bottom.saturating_add(PALETTE_GAP);
    let desired_circuit_y = separator_y.saturating_add(1 + SEPARATOR_TO_CIRCUIT_GAP);
    let total_circuit_height =
        (GATE_DRAW_HEIGHT * qubit_count as u16).saturating_add(ROW_GAP * (qubit_count as u16 - 1));
    let min_top = desired_circuit_y.saturating_sub(area.y);
    let available_state = area.height.saturating_sub(
        min_top
            .saturating_add(total_circuit_height)
            .saturating_add(1),
    );
    let max_states = (1usize << qubit_count).min(u16::MAX as usize) as u16;
    let target_states = if qubit_count <= MIN_QUBIT_COUNT {
        max_states.max(8)
    } else {
        max_states
    };
    let circles_height = target_states.min(available_state.max(1));
    let popup_height = if available_state > circles_height.saturating_add(7) {
        6
    } else {
        0
    };
    let popup_gap = if popup_height > 0 { 1 } else { 0 };
    let state_height = circles_height
        .saturating_add(popup_height)
        .saturating_add(popup_gap);
    let state_y = area
        .y
        .saturating_add(area.height.saturating_sub(state_height));
    let state_popup = Rect {
        x: area.x,
        y: state_y,
        width: area.width,
        height: popup_height,
    };
    let state_circles = Rect {
        x: area.x,
        y: state_y.saturating_add(popup_height + popup_gap),
        width: area.width,
        height: circles_height,
    };
    let max_circuit_y = state_y.saturating_sub(total_circuit_height + 1);
    let circuit_y = if max_circuit_y < desired_circuit_y {
        palette_bottom
    } else {
        desired_circuit_y
    };
    let mut circuits = Vec::with_capacity(qubit_count);
    for row in 0..qubit_count {
        let row_y = circuit_y.saturating_add(row as u16 * (GATE_DRAW_HEIGHT + ROW_GAP));
        circuits.push(Rect {
            x: area.x,
            y: row_y,
            width: area.width,
            height: GATE_DRAW_HEIGHT,
        });
    }
    Regions {
        palette: Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: palette_height,
        },
        circuits,
        state: Rect {
            x: area.x,
            y: state_y,
            width: area.width,
            height: state_height,
        },
        state_popup,
        state_circles,
    }
}

pub fn state_circle_layout(area: Rect, total: usize, qubits: usize) -> Option<StateCircleLayout> {
    if area.width == 0 || area.height == 0 || total == 0 {
        return None;
    }
    let (min_cell_w, min_cell_h) = if qubits == 2 {
        (12.0_f64, 9.0_f64)
    } else {
        (4.0_f64, 3.0_f64)
    };
    let min_cell_w = min_cell_w.min(area.width as f64).max(1.0);
    let min_cell_h = min_cell_h.min(area.height as f64).max(1.0);
    let max_cols = ((area.width as f64) / min_cell_w).floor().max(1.0) as usize;
    let max_rows = ((area.height as f64) / min_cell_h).floor().max(1.0) as usize;
    if max_cols == 0 || max_rows == 0 {
        return None;
    }
    let (columns, rows) = if qubits <= 1 && total <= 2 && max_cols >= total {
        (total.max(1), 1)
    } else {
        let columns_needed = total.div_ceil(max_rows);
        let columns = columns_needed.min(max_cols).max(1);
        let rows = total.div_ceil(columns).min(max_rows).max(1);
        (columns, rows)
    };
    let cell_w = area.width as f64 / columns as f64;
    let cell_h = area.height as f64 / rows as f64;
    let mut layout = StateCircleLayout {
        columns,
        rows,
        cell_w,
        cell_h,
        visible: rows * columns,
        origin_x: cell_w / 2.0,
        origin_y: cell_h / 2.0,
        step_x: cell_w,
        step_y: cell_h,
    };
    if qubits <= 1 && total <= 2 && max_cols >= total {
        let base_radius = state_circle_base_radius(&layout, qubits);
        let diameter = base_radius * 2.0;
        let gap = 2.0;
        let content_width = diameter * columns as f64 + gap * (columns.saturating_sub(1)) as f64;
        let offset = ((area.width as f64) - content_width).max(0.0) / 2.0;
        layout.step_x = diameter + gap;
        layout.origin_x = offset + diameter / 2.0;
    }
    Some(layout)
}

pub(crate) fn amplitude_qubits(len: usize) -> usize {
    let mut size = len.max(1);
    let mut qubits = 0;
    while size > 1 {
        size >>= 1;
        qubits += 1;
    }
    qubits
}

pub(crate) fn display_index_to_state_index(display_index: usize, qubits: usize) -> usize {
    let mut value = display_index;
    let mut reversed = 0usize;
    for _ in 0..qubits {
        reversed = (reversed << 1) | (value & 1);
        value >>= 1;
    }
    reversed
}

pub(crate) fn state_circle_base_radius(layout: &StateCircleLayout, qubits: usize) -> f64 {
    let size_boost = match qubits {
        0 | 1 => 1.9,
        2 => 2.4,
        3 => 1.0,
        4 => 0.9,
        _ => 0.8,
    };
    let min_cell = layout.cell_w.min(layout.cell_h);
    let mut base_radius = ((min_cell / 2.0) + 0.3) * size_boost;
    if qubits == 2 {
        base_radius = base_radius.min(min_cell * 0.52);
    }
    let max_radius = min_cell * 0.7;
    base_radius = base_radius.min(max_radius);
    let mut diameter = (base_radius * 2.0).round().max(1.0) as i32;
    if diameter % 2 == 0 {
        diameter += 1;
    }
    let mut max_diameter = (max_radius * 2.0).floor().max(1.0) as i32;
    if max_diameter % 2 == 0 {
        max_diameter = (max_diameter - 1).max(1);
    }
    if diameter > max_diameter {
        diameter = max_diameter;
    }
    diameter as f64 / 2.0
}

pub(crate) fn palette_items(area: Rect) -> Vec<PaletteItem> {
    let mut items = Vec::new();
    let columns = palette_columns(area.width) as usize;
    let stride_x = GATE_BOX_WIDTH.saturating_add(GATE_GAP);
    let stride_y = GATE_DRAW_HEIGHT.saturating_add(ROW_GAP);
    for (index, gate) in PALETTE_GATES.iter().copied().enumerate() {
        let row = index / columns;
        let col = index % columns;
        let rect = Rect {
            x: area.x.saturating_add((col as u16).saturating_mul(stride_x)),
            y: area.y.saturating_add((row as u16).saturating_mul(stride_y)),
            width: GATE_BOX_WIDTH,
            height: GATE_DRAW_HEIGHT,
        };
        items.push(PaletteItem { gate, rect });
    }
    items
}

fn palette_columns(width: u16) -> u16 {
    if width == 0 {
        return 1;
    }
    let stride = GATE_BOX_WIDTH.saturating_add(GATE_GAP);
    let columns = width.saturating_add(GATE_GAP) / stride;
    columns.max(1).min(PALETTE_GATES.len() as u16)
}

fn palette_height(width: u16) -> u16 {
    let columns = palette_columns(width);
    let total = PALETTE_GATES.len() as u16;
    let rows = total.div_ceil(columns).max(1);
    rows.saturating_mul(GATE_DRAW_HEIGHT)
        .saturating_add(rows.saturating_sub(1).saturating_mul(ROW_GAP))
}

pub fn circuit_layout(area: Rect, qubit_count: usize) -> CircuitLayout {
    let regions = layout_regions(area, qubit_count);
    let prefix_len = WIRE_PREFIX.chars().count() as u16;
    let wire_start_x = regions.circuits[0].x.saturating_add(prefix_len);
    let wire_width = regions.circuits[0].width.saturating_sub(prefix_len);
    let mut wire_rows = Vec::with_capacity(qubit_count);
    let mut slots = Vec::with_capacity(qubit_count);
    for row in 0..qubit_count {
        let circuit = regions.circuits[row];
        let wire_y = circuit.y.saturating_add(SHADOW_OUTSET + 1);
        wire_rows.push(wire_y);
        let mut row_slots = Vec::new();
        let mut x = wire_start_x.saturating_add(SLOT_PADDING_LEFT);
        let max_x = wire_start_x.saturating_add(wire_width);
        while x.saturating_add(GATE_BOX_WIDTH) <= max_x {
            row_slots.push(Rect {
                x,
                y: circuit.y,
                width: GATE_BOX_WIDTH,
                height: GATE_DRAW_HEIGHT,
            });
            x = x.saturating_add(GATE_BOX_WIDTH + SLOT_GAP);
        }
        slots.push(row_slots);
    }
    CircuitLayout {
        wire_start_x,
        wire_width,
        wire_rows,
        slots,
    }
}

pub fn hit_test_palette(x: u16, y: u16, area: Rect) -> Option<Gate> {
    let regions = layout_regions(area, MIN_QUBIT_COUNT);
    let items = palette_items(regions.palette);
    for item in items {
        if x >= item.rect.x
            && x < item.rect.x.saturating_add(item.rect.width)
            && y >= item.rect.y
            && y < item.rect.y.saturating_add(item.rect.height)
        {
            return Some(item.gate);
        }
    }
    None
}

pub fn hit_test_circuit_slot(
    x: u16,
    y: u16,
    area: Rect,
    qubit_count: usize,
) -> Option<(usize, usize)> {
    let layout = circuit_layout(area, qubit_count);
    for (row, row_slots) in layout.slots.iter().enumerate() {
        for (index, rect) in row_slots.iter().enumerate() {
            if x >= rect.x
                && x < rect.x.saturating_add(rect.width)
                && y >= rect.y
                && y < rect.y.saturating_add(rect.height)
            {
                return Some((row, index));
            }
        }
    }
    None
}

pub fn is_empty_drop(_x: u16, y: u16, area: Rect, qubit_count: usize) -> bool {
    let regions = layout_regions(area, qubit_count);
    let palette_bottom = regions.palette.y.saturating_add(regions.palette.height);
    let circuit_top = regions.circuits.first().map(|rect| rect.y).unwrap_or(0);
    let circuit_bottom = regions
        .circuits
        .last()
        .map(|rect| rect.y.saturating_add(rect.height))
        .unwrap_or(circuit_top);
    let within_palette = y >= regions.palette.y && y < palette_bottom;
    let within_circuit = y >= circuit_top && y < circuit_bottom;
    let within_state =
        y >= regions.state.y && y < regions.state.y.saturating_add(regions.state.height);
    !within_palette && !within_circuit && !within_state
}

pub fn insertion_snap_rect(layout: &CircuitLayout, row: usize, index: usize) -> Option<Rect> {
    let row_slots = layout.slots.get(row)?;
    if index == 0 {
        let first = row_slots.first()?;
        let gap_width = first.x.saturating_sub(layout.wire_start_x);
        if gap_width == 0 {
            return None;
        }
        let gap_center = layout.wire_start_x.saturating_add(gap_width / 2);
        let snap_x = gap_center.saturating_sub(GATE_BOX_WIDTH / 2);
        return Some(Rect {
            x: snap_x,
            y: first.y,
            width: GATE_BOX_WIDTH,
            height: GATE_DRAW_HEIGHT,
        });
    }
    let left = row_slots.get(index.saturating_sub(1))?;
    let gap_center = left.x.saturating_add(GATE_BOX_WIDTH + SLOT_GAP / 2);
    let snap_x = gap_center.saturating_sub(GATE_BOX_WIDTH / 2);
    Some(Rect {
        x: snap_x,
        y: left.y,
        width: GATE_BOX_WIDTH,
        height: GATE_DRAW_HEIGHT,
    })
}

pub fn column_line_x(layout: &CircuitLayout, row: usize, index: usize) -> Option<u16> {
    let slot = layout
        .slots
        .get(row)
        .and_then(|row_slots| row_slots.get(index))?;
    Some(
        slot.x
            .saturating_add(slot.width)
            .saturating_add(SLOT_GAP / 2),
    )
}

pub fn start_line_x(layout: &CircuitLayout) -> Option<u16> {
    let first_row = layout.slots.first()?;
    let first_slot = first_row.first()?;
    let padding = first_slot.x.saturating_sub(layout.wire_start_x);
    Some(layout.wire_start_x.saturating_add(padding / 2))
}

pub fn hovered_start_at(x: u16, y: u16, layout: &CircuitLayout) -> bool {
    let first_row = match layout.slots.first() {
        Some(row) => row,
        None => return false,
    };
    let first_slot = match first_row.first() {
        Some(slot) => slot,
        None => return false,
    };
    let last_row = match layout.slots.last() {
        Some(row) => row,
        None => return false,
    };
    let last_slot = match last_row.first() {
        Some(slot) => slot,
        None => return false,
    };
    let top = first_slot.y;
    let bottom = last_slot.y.saturating_add(last_slot.height);
    y >= top && y < bottom && x < first_slot.x
}

pub fn hovered_column_at(x: u16, y: u16, layout: &CircuitLayout) -> Option<(usize, usize)> {
    let first_row = layout.slots.first()?;
    let first_slot = first_row.first()?;
    let last_row = layout.slots.last()?;
    let last_slot = last_row.first()?;
    let top = first_slot.y;
    let bottom = last_slot.y.saturating_add(last_slot.height);
    if y < top || y >= bottom {
        return None;
    }
    for (index, rect) in first_row.iter().enumerate() {
        if x >= rect.x && x < rect.x.saturating_add(rect.width) {
            return Some((0, index));
        }
    }
    let last_index = first_row.len().saturating_sub(1);
    for (index, left) in first_row.iter().copied().enumerate().take(last_index) {
        let gap_x = left.x.saturating_add(GATE_BOX_WIDTH);
        let gap = Rect {
            x: gap_x,
            y: left.y,
            width: SLOT_GAP,
            height: left.height,
        };
        if x >= gap.x && x < gap.x.saturating_add(gap.width) {
            return Some((0, index));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_regions_keeps_state_height_for_16_qubits() {
        let area = Rect::new(0, 0, 120, 120);
        let regions = layout_regions(area, 16);
        assert!(regions.state.height > 0);
    }

    #[test]
    fn display_index_to_state_index_reverses_bits() {
        assert_eq!(
            (
                display_index_to_state_index(0, 2),
                display_index_to_state_index(1, 2),
                display_index_to_state_index(2, 2),
                display_index_to_state_index(3, 2),
            ),
            (0, 2, 1, 3)
        );
    }
}
