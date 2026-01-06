use ratatui::layout::Rect;

use crate::{
    GATE_BOX_WIDTH, GATE_DRAW_HEIGHT, GATE_GAP, MIN_QUBIT_COUNT, PALETTE_GAP, PALETTE_HEIGHT,
    ROW_GAP, SEPARATOR_TO_CIRCUIT_GAP, SHADOW_OUTSET, SLOT_GAP,
};
use crate::model::Gate;

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
}

#[derive(Debug)]
pub struct CircuitLayout {
    pub wire_start_x: u16,
    pub wire_width: u16,
    pub wire_rows: Vec<u16>,
    pub slots: Vec<Vec<Rect>>,
}

pub fn layout_regions(area: Rect, qubit_count: usize) -> Regions {
    let palette_bottom = area.y.saturating_add(PALETTE_HEIGHT);
    let separator_y = palette_bottom.saturating_add(PALETTE_GAP);
    let desired_circuit_y = separator_y.saturating_add(1 + SEPARATOR_TO_CIRCUIT_GAP);
    let total_circuit_height =
        (GATE_DRAW_HEIGHT * qubit_count as u16).saturating_add(ROW_GAP * (qubit_count as u16 - 1));
    let min_top = desired_circuit_y.saturating_sub(area.y);
    let available_state = area
        .height
        .saturating_sub(min_top.saturating_add(total_circuit_height).saturating_add(1));
    let max_states = (1usize << qubit_count) as u16;
    let state_height = max_states.min(available_state.max(1));
    let state_y = area.y.saturating_add(area.height.saturating_sub(state_height));
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
            height: PALETTE_HEIGHT,
        },
        circuits,
        state: Rect {
            x: area.x,
            y: state_y,
            width: area.width,
            height: state_height,
        },
    }
}

pub(crate) fn palette_items(area: Rect) -> Vec<PaletteItem> {
    let mut items = Vec::new();
    let mut x = area.x;
    let y = area.y;
    for gate in PALETTE_GATES {
        let rect = Rect {
            x,
            y,
            width: GATE_BOX_WIDTH,
            height: GATE_DRAW_HEIGHT,
        };
        items.push(PaletteItem { gate, rect });
        x = x.saturating_add(GATE_BOX_WIDTH + GATE_GAP);
    }
    items
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
    let within_state = y >= regions.state.y && y < regions.state.y.saturating_add(regions.state.height);
    !within_palette && !within_circuit && !within_state
}

pub fn insertion_snap_rect(layout: &CircuitLayout, row: usize, index: usize) -> Option<Rect> {
    let left = layout
        .slots
        .get(row)
        .and_then(|slots| slots.get(index.saturating_sub(1)))?;
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
    for index in 0..first_row.len().saturating_sub(1) {
        let left = first_row[index];
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
