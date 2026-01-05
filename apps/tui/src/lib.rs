use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Text;
use ratatui::widgets::{Paragraph, Widget};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Gate {
    X,
    H,
    Y,
    Z,
    S,
    T,
}

const PALETTE_GATES: [Gate; 6] = [Gate::H, Gate::X, Gate::Y, Gate::Z, Gate::S, Gate::T];
const PALETTE_LABEL: &str = "";
const GATE_BOX_WIDTH: u16 = 5;
const GATE_BOX_HEIGHT: u16 = 3;
const PALETTE_HEIGHT: u16 = GATE_BOX_HEIGHT;
const PALETTE_GAP: u16 = 1;
const SEPARATOR_TO_CIRCUIT_GAP: u16 = 1;
const GATE_GAP: u16 = 1;
const WIRE_PREFIX: &str = "q0: ";
const SLOT_PADDING_LEFT: u16 = 3;
const SLOT_GAP: u16 = 3;
const SNAP_DISTANCE: u16 = 1;
const MIN_QUBIT_COUNT: usize = 2;
const ROW_GAP: u16 = 1;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum DragOrigin {
    Palette,
    Circuit,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct DragState {
    pub gate: Gate,
    pub origin: DragOrigin,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub placed: Vec<Vec<Option<Gate>>>,
    pub dragging: Option<DragState>,
    pub drag_pos: Option<(u16, u16)>,
    pub default_gate: Gate,
    pub initialized: bool,
    pub hovered_slot: Option<usize>,
    pub hovered_row: Option<usize>,
    pub hovered_insert: Option<(usize, usize)>,
}

impl AppState {
    pub fn new(initial_gate: Gate) -> Self {
        Self {
            placed: vec![Vec::new(); MIN_QUBIT_COUNT],
            dragging: None,
            drag_pos: None,
            default_gate: initial_gate,
            initialized: false,
            hovered_slot: None,
            hovered_row: None,
            hovered_insert: None,
        }
    }
}

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

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct DragVisual {
    pub gate: Gate,
    pub x: u16,
    pub y: u16,
}

#[derive(Debug)]
pub struct CircuitLayout {
    pub wire_start_x: u16,
    pub wire_width: u16,
    pub wire_rows: Vec<u16>,
    pub slots: Vec<Vec<Rect>>,
}

fn qubit_count(state: &AppState) -> usize {
    state.placed.len().max(MIN_QUBIT_COUNT)
}

pub fn layout_regions(area: Rect, qubit_count: usize) -> Regions {
    let state_y = area.y.saturating_add(area.height.saturating_sub(1));
    let palette_bottom = area.y.saturating_add(PALETTE_HEIGHT);
    let separator_y = palette_bottom.saturating_add(PALETTE_GAP);
    let desired_circuit_y = separator_y.saturating_add(1 + SEPARATOR_TO_CIRCUIT_GAP);
    let total_circuit_height =
        (GATE_BOX_HEIGHT * qubit_count as u16).saturating_add(ROW_GAP * (qubit_count as u16 - 1));
    let max_circuit_y = state_y.saturating_sub(total_circuit_height + 1);
    let circuit_y = if max_circuit_y < palette_bottom {
        palette_bottom
    } else {
        desired_circuit_y.min(max_circuit_y)
    };
    let mut circuits = Vec::with_capacity(qubit_count);
    for row in 0..qubit_count {
        let row_y = circuit_y.saturating_add(row as u16 * (GATE_BOX_HEIGHT + ROW_GAP));
        circuits.push(Rect {
            x: area.x,
            y: row_y,
            width: area.width,
            height: GATE_BOX_HEIGHT,
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
            height: 1,
        },
    }
}

fn palette_items(area: Rect) -> Vec<PaletteItem> {
    let mut items = Vec::new();
    let mut x = area.x;
    let y = area.y;
    for gate in PALETTE_GATES {
        let rect = Rect {
            x,
            y,
            width: GATE_BOX_WIDTH,
            height: GATE_BOX_HEIGHT,
        };
        items.push(PaletteItem { gate, rect });
        x = x.saturating_add(GATE_BOX_WIDTH + GATE_GAP);
    }
    items
}

fn gate_theme(gate: Gate) -> (Color, Color, Color, Color) {
    let background = match gate {
        Gate::H => Color::Yellow,
        Gate::X => Color::Red,
        Gate::Y => Color::Magenta,
        Gate::Z => Color::Blue,
        Gate::S => Color::Green,
        Gate::T => Color::Cyan,
    };
    let text = Color::Black;
    let highlight = Color::White;
    let shadow = match background {
        Color::Yellow => Color::LightYellow,
        Color::Red => Color::LightRed,
        Color::Magenta => Color::LightMagenta,
        Color::Blue => Color::LightBlue,
        Color::Green => Color::LightGreen,
        Color::Cyan => Color::LightCyan,
        _ => Color::DarkGray,
    };
    (text, background, highlight, shadow)
}

fn draw_gate_box(buffer: &mut Buffer, rect: Rect, gate: Gate) {
    if rect.width < GATE_BOX_WIDTH || rect.height < GATE_BOX_HEIGHT {
        return;
    }
    let (text, background, _highlight, shadow) = gate_theme(gate);
    let base_style = Style::default().fg(text).bg(background);
    for offset in 0..rect.height {
        let line = " ".repeat(rect.width as usize);
        buffer.set_string(rect.x, rect.y + offset, line, base_style);
    }
    if rect.height > 2 {
        let top = "▔".repeat(rect.width as usize);
        buffer.set_string(
            rect.x,
            rect.y,
            top,
            Style::default().fg(shadow).bg(background),
        );
    }
    if rect.height > 1 {
        let bottom = "▁".repeat(rect.width as usize);
        buffer.set_string(
            rect.x,
            rect.y + rect.height - 1,
            bottom,
            Style::default().fg(shadow).bg(background),
        );
    }
    if rect.width > 2 && rect.height > 2 {
        let fill = " ".repeat(rect.width as usize);
        for row in 1..rect.height - 1 {
            buffer.set_string(rect.x, rect.y + row, &fill, base_style);
        }
    }
    let label_x = rect.x + rect.width / 2;
    let label_y = rect.y + rect.height / 2;
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

fn draw_slot_placeholder(buffer: &mut Buffer, rect: Rect, color: Color) {
    if rect.width < GATE_BOX_WIDTH || rect.height < GATE_BOX_HEIGHT {
        return;
    }
    let outline = Style::default().bg(color);
    let line = " ".repeat(rect.width as usize);
    for row in 0..rect.height {
        buffer.set_string(rect.x, rect.y + row, &line, outline);
    }
}

fn circuit_layout(area: Rect, qubit_count: usize) -> CircuitLayout {
    let regions = layout_regions(area, qubit_count);
    let prefix_len = WIRE_PREFIX.chars().count() as u16;
    let wire_start_x = regions.circuits[0].x.saturating_add(prefix_len);
    let wire_width = regions.circuits[0].width.saturating_sub(prefix_len);
    let mut wire_rows = Vec::with_capacity(qubit_count);
    let mut slots = Vec::with_capacity(qubit_count);
    for row in 0..qubit_count {
        let circuit = regions.circuits[row];
        let wire_y = circuit.y.saturating_add(1);
        wire_rows.push(wire_y);
        let mut row_slots = Vec::new();
        let mut x = wire_start_x.saturating_add(SLOT_PADDING_LEFT);
        let max_x = wire_start_x.saturating_add(wire_width);
        while x.saturating_add(GATE_BOX_WIDTH) <= max_x {
            row_slots.push(Rect {
                x,
                y: circuit.y,
                width: GATE_BOX_WIDTH,
                height: GATE_BOX_HEIGHT,
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
    !within_palette && !within_circuit && y != regions.state.y
}

fn ensure_slots(state: &mut AppState, counts: &[usize]) {
    for (row, &count) in counts.iter().enumerate() {
        if state.placed.len() <= row {
            state.placed.push(Vec::new());
        }
        state.placed[row].resize(count, None);
    }
    if !state.initialized && !counts.is_empty() && counts[0] > 0 {
        state.placed[0][0] = Some(state.default_gate);
        state.initialized = true;
    }
}

fn add_empty_qubit_row(state: &mut AppState) {
    state.placed.push(Vec::new());
}

fn compact_empty_columns(state: &mut AppState) {
    let max_len = state
        .placed
        .iter()
        .map(|row| row.len())
        .max()
        .unwrap_or(0);
    if max_len == 0 {
        return;
    }

    let mut new_rows = vec![Vec::new(); state.placed.len()];
    for col in 0..max_len {
        let mut has_gate = false;
        for row in &state.placed {
            if row.get(col).and_then(|gate| *gate).is_some() {
                has_gate = true;
                break;
            }
        }
        if !has_gate {
            continue;
        }
        for (row_index, row) in state.placed.iter().enumerate() {
            let value = row.get(col).and_then(|gate| *gate);
            new_rows[row_index].push(value);
        }
    }
    state.placed = new_rows;
}

fn trim_trailing_empty_qubits(state: &mut AppState) {
    while state.placed.len() > MIN_QUBIT_COUNT {
        let remove = state
            .placed
            .last()
            .map(|row| row.iter().all(|gate| gate.is_none()))
            .unwrap_or(false);
        if remove {
            state.placed.pop();
        } else {
            break;
        }
    }
}

fn insertion_target(
    state: &AppState,
    x: u16,
    y: u16,
    area: Rect,
) -> Option<(usize, usize)> {
    let layout = circuit_layout(area, qubit_count(state));
    let mut best: Option<(usize, usize, u16)> = None;
    for (row, row_slots) in layout.slots.iter().enumerate() {
        for index in 0..row_slots.len().saturating_sub(1) {
            let left = row_slots[index];
            let gap_x = left.x.saturating_add(GATE_BOX_WIDTH);
            let gap = Rect {
                x: gap_x,
                y: left.y,
                width: SLOT_GAP,
                height: left.height,
            };
            let left_gate = state
                .placed
                .get(row)
                .and_then(|row_gates| row_gates.get(index))
                .and_then(|value| *value);
            let right_gate = state
                .placed
                .get(row)
                .and_then(|row_gates| row_gates.get(index + 1))
                .and_then(|value| *value);
            if left_gate.is_none() || right_gate.is_none() {
                continue;
            }
            let dx = if x < gap.x {
                gap.x - x
            } else if x >= gap.x.saturating_add(gap.width) {
                x - gap.x.saturating_add(gap.width - 1)
            } else {
                0
            };
            let dy = if y < gap.y {
                gap.y - y
            } else if y >= gap.y.saturating_add(gap.height) {
                y - gap.y.saturating_add(gap.height - 1)
            } else {
                0
            };
            let dist = dx.saturating_add(dy);
            if dist > SNAP_DISTANCE {
                continue;
            }
            if best.map_or(true, |(_, _, best_dist)| dist < best_dist) {
                best = Some((row, index + 1, dist));
            }
        }
    }
    best.map(|(row, index, _)| (row, index))
}

fn insertion_snap_rect(layout: &CircuitLayout, row: usize, index: usize) -> Option<Rect> {
    let left = layout.slots.get(row).and_then(|slots| slots.get(index.saturating_sub(1)))?;
    let gap_center = left.x.saturating_add(GATE_BOX_WIDTH + SLOT_GAP / 2);
    let snap_x = gap_center.saturating_sub(GATE_BOX_WIDTH / 2);
    Some(Rect {
        x: snap_x,
        y: left.y,
        width: GATE_BOX_WIDTH,
        height: GATE_BOX_HEIGHT,
    })
}

pub fn handle_mouse_down(state: &mut AppState, x: u16, y: u16, area: Rect) {
    if let Some(gate) = hit_test_palette(x, y, area) {
        state.dragging = Some(DragState {
            gate,
            origin: DragOrigin::Palette,
        });
        state.drag_pos = Some((x, y));
        state.hovered_insert = None;
        add_empty_qubit_row(state);
        return;
    }

    let layout = circuit_layout(area, qubit_count(state));
    let counts: Vec<usize> = layout
        .slots
        .iter()
        .map(|row_slots| row_slots.len())
        .collect();
    ensure_slots(state, &counts);
    if let Some((row, slot)) = hit_test_circuit_slot(x, y, area, qubit_count(state)) {
        if let Some(gate) = state
            .placed
            .get(row)
            .and_then(|row_slots| row_slots.get(slot))
            .and_then(|value| *value)
        {
            state.dragging = Some(DragState {
                gate,
                origin: DragOrigin::Circuit,
            });
            state.drag_pos = Some((x, y));
            state.placed[row][slot] = None;
            state.hovered_insert = None;
            add_empty_qubit_row(state);
        }
    }
}

pub fn handle_mouse_move(state: &mut AppState, x: u16, y: u16) {
    if state.dragging.is_some() {
        state.drag_pos = Some((x, y));
    }
}

pub fn handle_mouse_up(state: &mut AppState, x: u16, y: u16, area: Rect) {
    let Some(dragging) = state.dragging else {
        return;
    };

    let layout = circuit_layout(area, qubit_count(state));
    let counts: Vec<usize> = layout
        .slots
        .iter()
        .map(|row_slots| row_slots.len())
        .collect();
    ensure_slots(state, &counts);
    if let Some((row, index)) = insertion_target(state, x, y, area) {
        for row_slots in &mut state.placed {
            let insert_at = index.min(row_slots.len());
            row_slots.insert(insert_at, None);
        }
        state.placed[row][index] = Some(dragging.gate);
    } else if let Some((row, slot)) = hit_test_circuit_slot(x, y, area, qubit_count(state)) {
        state.placed[row][slot] = Some(dragging.gate);
    } else if is_empty_drop(x, y, area, qubit_count(state))
        && dragging.origin == DragOrigin::Circuit
    {
        // already cleared on drag start
    }

    state.dragging = None;
    state.drag_pos = None;
    state.hovered_slot = None;
    state.hovered_row = None;
    state.hovered_insert = None;
    compact_empty_columns(state);
    trim_trailing_empty_qubits(state);
}

pub fn update_hovered_slot(state: &mut AppState, x: u16, y: u16, area: Rect) {
    if state.dragging.is_none() {
        state.hovered_slot = None;
        state.hovered_row = None;
        state.hovered_insert = None;
        return;
    }

    let layout = circuit_layout(area, qubit_count(state));
    let counts: Vec<usize> = layout
        .slots
        .iter()
        .map(|row_slots| row_slots.len())
        .collect();
    ensure_slots(state, &counts);
    let mut best: Option<(usize, u16)> = None;
    let mut best_row: Option<usize> = None;
    for (row, row_slots) in layout.slots.iter().enumerate() {
        for (index, rect) in row_slots.iter().enumerate() {
            if state.placed[row]
                .get(index)
                .and_then(|value| *value)
                .is_some()
            {
                continue;
            }
            let dx = if x < rect.x {
                rect.x - x
            } else if x >= rect.x.saturating_add(rect.width) {
                x - rect.x.saturating_add(rect.width - 1)
            } else {
                0
            };
            let dy = if y < rect.y {
                rect.y - y
            } else if y >= rect.y.saturating_add(rect.height) {
                y - rect.y.saturating_add(rect.height - 1)
            } else {
                0
            };
            let dist = dx.saturating_add(dy);
            if dist > SNAP_DISTANCE {
                continue;
            }
            if best.is_none_or(|(_, best_dist)| dist < best_dist) {
                best = Some((index, dist));
                best_row = Some(row);
            }
        }
    }
    let slot_target = match (best, best_row) {
        (Some((index, dist)), Some(row)) => Some((row, index, dist)),
        _ => None,
    };
    let insert_target = insertion_target(state, x, y, area).and_then(|(row, index)| {
        let layout = circuit_layout(area, qubit_count(state));
        let left = layout.slots.get(row).and_then(|slots| slots.get(index.saturating_sub(1)))?;
        let gap_x = left.x.saturating_add(GATE_BOX_WIDTH);
        let gap = Rect {
            x: gap_x,
            y: left.y,
            width: SLOT_GAP,
            height: left.height,
        };
        let dx = if x < gap.x {
            gap.x - x
        } else if x >= gap.x.saturating_add(gap.width) {
            x - gap.x.saturating_add(gap.width - 1)
        } else {
            0
        };
        let dy = if y < gap.y {
            gap.y - y
        } else if y >= gap.y.saturating_add(gap.height) {
            y - gap.y.saturating_add(gap.height - 1)
        } else {
            0
        };
        Some((row, index, dx.saturating_add(dy)))
    });

    if let Some((row, index, _)) = insert_target {
        state.hovered_slot = None;
        state.hovered_row = None;
        state.hovered_insert = Some((row, index));
        return;
    }

    state.hovered_slot = slot_target.map(|(_, index, _)| index);
    state.hovered_row = slot_target.map(|(row, _, _)| row);
    state.hovered_insert = None;
}

impl std::str::FromStr for Gate {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_uppercase().as_str() {
            "X" => Ok(Self::X),
            "H" => Ok(Self::H),
            "Y" => Ok(Self::Y),
            "Z" => Ok(Self::Z),
            "S" => Ok(Self::S),
            "T" => Ok(Self::T),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for Gate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::X => "X",
            Self::H => "H",
            Self::Y => "Y",
            Self::Z => "Z",
            Self::S => "S",
            Self::T => "T",
        };
        f.write_str(label)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

const INV_SQRT2: f64 = 0.7071067811865475_f64;
const PHASE_45: Complex = Complex {
    re: INV_SQRT2,
    im: INV_SQRT2,
};

fn matrix_for(gate: Gate) -> [Complex; 4] {
    match gate {
        Gate::X => [
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 1.0, im: 0.0 },
            Complex { re: 1.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
        ],
        Gate::H => [
            Complex {
                re: INV_SQRT2,
                im: 0.0,
            },
            Complex {
                re: INV_SQRT2,
                im: 0.0,
            },
            Complex {
                re: INV_SQRT2,
                im: 0.0,
            },
            Complex {
                re: -INV_SQRT2,
                im: 0.0,
            },
        ],
        Gate::Y => [
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 0.0, im: -1.0 },
            Complex { re: 0.0, im: 1.0 },
            Complex { re: 0.0, im: 0.0 },
        ],
        Gate::Z => [
            Complex { re: 1.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: -1.0, im: 0.0 },
        ],
        Gate::S => [
            Complex { re: 1.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 0.0, im: 1.0 },
        ],
        Gate::T => [
            Complex { re: 1.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            PHASE_45,
        ],
    }
}

fn mul(a: Complex, b: Complex) -> Complex {
    Complex {
        re: a.re * b.re - a.im * b.im,
        im: a.re * b.im + a.im * b.re,
    }
}

fn add(a: Complex, b: Complex) -> Complex {
    Complex {
        re: a.re + b.re,
        im: a.im + b.im,
    }
}

pub fn apply_gate_to_state(state: [Complex; 4], gate: Gate, target: usize) -> [Complex; 4] {
    let [a00, a01, a10, a11] = matrix_for(gate);
    let mut out = state;
    match target {
        0 => {
            for &(i0, i1) in &[(0, 2), (1, 3)] {
                let v0 = out[i0];
                let v1 = out[i1];
                out[i0] = add(mul(a00, v0), mul(a01, v1));
                out[i1] = add(mul(a10, v0), mul(a11, v1));
            }
        }
        _ => {
            for &(i0, i1) in &[(0, 1), (2, 3)] {
                let v0 = out[i0];
                let v1 = out[i1];
                out[i0] = add(mul(a00, v0), mul(a01, v1));
                out[i1] = add(mul(a10, v0), mul(a11, v1));
            }
        }
    }
    out
}

pub fn apply_gates_to_zero(gates: &[Vec<Option<Gate>>]) -> [Complex; 4] {
    let mut state = [
        Complex { re: 1.0, im: 0.0 },
        Complex { re: 0.0, im: 0.0 },
        Complex { re: 0.0, im: 0.0 },
        Complex { re: 0.0, im: 0.0 },
    ];
    let max_slots = gates.iter().map(|row| row.len()).max().unwrap_or(0);
    for slot in 0..max_slots {
        for (row, row_gates) in gates.iter().enumerate() {
            if row >= MIN_QUBIT_COUNT {
                continue;
            }
            if let Some(Some(gate)) = row_gates.get(slot) {
                state = apply_gate_to_state(state, *gate, row);
            }
        }
    }
    state
}

fn normalize_zero(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

pub fn format_complex(value: Complex) -> String {
    let re = normalize_zero(value.re);
    let im = normalize_zero(value.im);
    let sign = if im < 0.0 { '-' } else { '+' };
    let abs_im = im.abs();
    format!("{}{}{}i", re, sign, abs_im)
}

pub fn build_state_line(gates: &[Vec<Option<Gate>>]) -> String {
    let [amp0, amp1, amp2, amp3] = apply_gates_to_zero(gates);
    format!(
        "State: [({}), ({}), ({}), ({})]",
        format_complex(amp0),
        format_complex(amp1),
        format_complex(amp2),
        format_complex(amp3)
    )
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
            Style::default(),
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
            Style::default().fg(Color::DarkGray),
        );
    }

    let wire_line = "-".repeat(layout.wire_width as usize);
    for row in 0..current_qubits {
        if let Some(wire_y) = layout.wire_rows.get(row) {
            buffer.set_string(
                regions.circuits[row].x,
                *wire_y,
                format!("q{}: {}", row, wire_line),
                Style::default(),
            );
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
    if state.dragging.is_some() {
        for (row, row_slots) in layout.slots.iter().enumerate() {
            for (slot, rect) in row_slots.iter().enumerate() {
                if state.placed[row]
                    .get(slot)
                    .and_then(|value| *value)
                    .is_none()
                {
                    draw_slot_placeholder(&mut buffer, *rect, Color::DarkGray);
                }
            }
        }
        if let (Some(row), Some(slot)) = (state.hovered_row, state.hovered_slot) {
            if state.placed[row]
                .get(slot)
                .and_then(|value| *value)
                .is_none()
            {
                if let Some(rect) = layout
                    .slots
                    .get(row)
                    .and_then(|row_slots| row_slots.get(slot))
                {
                    draw_slot_placeholder(&mut buffer, *rect, Color::Black);
                }
            }
        }
    }
    if let Some(debug) = debug_line {
        if !debug.trim().is_empty() {
            let text = Text::from(format!("Debug: {}", debug));
            let paragraph = Paragraph::new(text);
            let debug_area = Rect {
                x: area.x,
                y: regions.state.y.saturating_sub(1),
                width: area.width,
                height: 1,
            };
            paragraph.render(debug_area, &mut buffer);
        }
    }
    let state_line = build_state_line(&state.placed);
    let text = Text::from(state_line);
    let paragraph = Paragraph::new(text);
    paragraph.render(regions.state, &mut buffer);
    if let Some(drag) = drag {
        let mut rect = Rect {
            x: drag.x,
            y: drag.y,
            width: GATE_BOX_WIDTH,
            height: GATE_BOX_HEIGHT,
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
            draw_gate_box(&mut buffer, rect, drag.gate);
        }
    }
    buffer
}

pub fn parse_args(args: &[String]) -> Gate {
    let mut gate_value: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        let current = &args[i];
        if current == "--gate" && i + 1 < args.len() {
            gate_value = Some(args[i + 1].clone());
            i += 2;
            continue;
        }
        if let Some(rest) = current.strip_prefix("--gate=") {
            gate_value = Some(rest.to_string());
        }
        i += 1;
    }

    gate_value
        .as_deref()
        .and_then(|value| value.parse().ok())
        .unwrap_or(Gate::H)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn parse_args_from(args: &[&str]) -> Gate {
        let owned: Vec<String> = args.iter().map(|value| value.to_string()).collect();
        parse_args(&owned)
    }

    #[test]
    fn build_state_line_defaults_to_zero_state() {
        let line = build_state_line(&[]);
        assert_eq!(line, "State: [(1+0i), (0+0i), (0+0i), (0+0i)]");
    }

    #[test]
    fn build_state_line_for_h_gate() {
        let line = build_state_line(&[vec![Some(Gate::H)]]);
        assert_eq!(
            line,
            "State: [(0.7071067811865475+0i), (0+0i), (0.7071067811865475+0i), (0+0i)]"
        );
    }

    #[test]
    fn build_state_line_for_y_gate() {
        let line = build_state_line(&[vec![Some(Gate::Y)]]);
        assert_eq!(line, "State: [(0+0i), (0+0i), (0+1i), (0+0i)]");
    }

    #[test]
    fn parse_args_handles_gate_flags() {
        assert_eq!(parse_args_from(&[]), Gate::H);
        assert_eq!(parse_args_from(&["--gate", "y"]), Gate::Y);
        assert_eq!(parse_args_from(&["--gate=Z"]), Gate::Z);
        assert_eq!(parse_args_from(&["--gate", "nope"]), Gate::H);
    }

    #[test]
    fn render_to_buffer_writes_spaced_lines() {
        let area = Rect::new(0, 0, 100, 12);
        let mut state = AppState::new(Gate::H);
        let buffer = render_to_buffer(&mut state, area, None);
        let line0 = buffer_to_line(&buffer, area, 0);
        let layout = circuit_layout(area, qubit_count(&state));
        let gate_rect = layout.slots[0][0];
        let top = buffer_to_string(&buffer, gate_rect.x, gate_rect.y, GATE_BOX_WIDTH);
        let mid = buffer_to_string(&buffer, gate_rect.x, gate_rect.y + 1, GATE_BOX_WIDTH);
        let state_line = buffer_to_line(&buffer, area, area.height - 1);
        let wire_line = buffer_to_line(&buffer, area, layout.wire_rows[0]);
        assert!(!line0.trim().is_empty());
        assert_eq!(top, "▔▔▔▔▔");
        assert_eq!(mid, "  H  ");
        assert!(wire_line.starts_with("q0: "));
        assert_eq!(
            state_line,
            "State: [(0.7071067811865475+0i), (0+0i), (0.7071067811865475+0i), (0+0i)]"
        );
    }

    #[test]
    fn render_to_buffer_shows_drag_overlay() {
        let area = Rect::new(0, 0, 20, 12);
        let drag = DragVisual {
            gate: Gate::X,
            x: 5,
            y: 3,
        };
        let mut state = AppState::new(Gate::H);
        let buffer = render_to_buffer_with_drag(&mut state, area, None, Some(drag));
        let overlay = buffer_to_string(&buffer, 5, 3, 5);
        assert_eq!(overlay, "▔▔▔▔▔");
    }

    #[test]
    fn hover_slot_draws_outline() {
        let area = Rect::new(0, 0, 60, 10);
        let mut state = AppState::new(Gate::H);
        state.hovered_slot = Some(1);
        state.hovered_row = Some(0);
        state.dragging = Some(DragState {
            gate: Gate::X,
            origin: DragOrigin::Palette,
        });
        let buffer = render_to_buffer_with_drag(&mut state, area, None, None);
        let layout = circuit_layout(area, qubit_count(&state));
        let slot = layout.slots[0][1];
        let outline = buffer_to_string(&buffer, slot.x, slot.y, GATE_BOX_WIDTH);
        assert_eq!(outline, "     ");
    }

    #[test]
    fn drag_visual_snaps_to_hovered_slot() {
        let area = Rect::new(0, 0, 60, 10);
        let mut state = AppState::new(Gate::H);
        state.hovered_slot = Some(1);
        state.hovered_row = Some(0);
        state.dragging = Some(DragState {
            gate: Gate::X,
            origin: DragOrigin::Palette,
        });
        let drag = DragVisual {
            gate: Gate::X,
            x: 1,
            y: 1,
        };
        let buffer = render_to_buffer_with_drag(&mut state, area, None, Some(drag));
        let layout = circuit_layout(area, qubit_count(&state));
        let slot = layout.slots[0][1];
        let top = buffer_to_string(&buffer, slot.x, slot.y, GATE_BOX_WIDTH);
        assert_eq!(top, "▔▔▔▔▔");
    }

    #[test]
    fn drag_visual_does_not_snap_to_occupied_slot() {
        let area = Rect::new(0, 0, 60, 10);
        let mut state = AppState::new(Gate::H);
        state.hovered_slot = Some(1);
        state.hovered_row = Some(0);
        state.dragging = Some(DragState {
            gate: Gate::X,
            origin: DragOrigin::Palette,
        });
        ensure_slots(&mut state, &[3, 3]);
        state.placed[0][1] = Some(Gate::Z);
        let drag = DragVisual {
            gate: Gate::X,
            x: 1,
            y: 1,
        };
        let buffer = render_to_buffer_with_drag(&mut state, area, None, Some(drag));
        let overlay_top = buffer_to_string(&buffer, 1, 1, GATE_BOX_WIDTH);
        let overlay_mid = buffer_to_string(&buffer, 1, 2, GATE_BOX_WIDTH);
        assert_eq!(overlay_top, "▔▔▔▔▔");
        assert_eq!(overlay_mid, "  X  ");
        let layout = circuit_layout(area, qubit_count(&state));
        let slot = layout.slots[0][1];
        let slot_mid = buffer_to_string(&buffer, slot.x, slot.y + 1, GATE_BOX_WIDTH);
        assert_eq!(slot_mid, "  Z  ");
    }

    #[test]
    fn hit_test_palette_finds_gate() {
        let area = Rect::new(0, 0, 60, 6);
        let state = AppState::new(Gate::H);
        let regions = layout_regions(area, qubit_count(&state));
        let items = palette_items(regions.palette);
        let target = items.iter().find(|item| item.gate == Gate::X).unwrap();
        let x = target.rect.x;
        let gate = hit_test_palette(x, target.rect.y, area);
        assert_eq!(gate, Some(Gate::X));
    }

    #[test]
    fn grabbing_gate_adds_empty_qubit_row() {
        let area = Rect::new(0, 0, 60, 10);
        let mut state = AppState::new(Gate::H);
        let initial_rows = state.placed.len();
        let regions = layout_regions(area, qubit_count(&state));
        let items = palette_items(regions.palette);
        let target = items.iter().find(|item| item.gate == Gate::H).unwrap();
        handle_mouse_down(&mut state, target.rect.x, target.rect.y, area);
        assert_eq!(state.placed.len(), initial_rows + 1);
    }

    #[test]
    fn drop_clears_empty_trailing_qubit() {
        let area = Rect::new(0, 0, 60, 10);
        let mut state = AppState::new(Gate::H);
        let regions = layout_regions(area, qubit_count(&state));
        let items = palette_items(regions.palette);
        let target = items.iter().find(|item| item.gate == Gate::Z).unwrap();
        handle_mouse_down(&mut state, target.rect.x, target.rect.y, area);
        let layout = circuit_layout(area, qubit_count(&state));
        let slot = layout.slots[0][0];
        handle_mouse_up(&mut state, slot.x, slot.y, area);
        assert_eq!(state.placed.len(), MIN_QUBIT_COUNT);
    }

    #[test]
    fn drop_on_bottom_qubit_keeps_row() {
        let area = Rect::new(0, 0, 60, 12);
        let mut state = AppState::new(Gate::H);
        let regions = layout_regions(area, qubit_count(&state));
        let items = palette_items(regions.palette);
        let target = items.iter().find(|item| item.gate == Gate::Z).unwrap();
        handle_mouse_down(&mut state, target.rect.x, target.rect.y, area);
        let layout = circuit_layout(area, qubit_count(&state));
        let last_row = layout.slots.len() - 1;
        let slot = layout.slots[last_row][0];
        handle_mouse_up(&mut state, slot.x, slot.y, area);
        assert_eq!(state.placed.len(), MIN_QUBIT_COUNT + 1);
    }

    #[test]
    fn drop_compacts_empty_columns() {
        let area = Rect::new(0, 0, 80, 12);
        let mut state = AppState::new(Gate::H);
        let layout = circuit_layout(area, qubit_count(&state));
        let slot0 = layout.slots[0][0];
        let slot1 = layout.slots[0][1];
        handle_mouse_down(&mut state, slot0.x, slot0.y, area);
        handle_mouse_up(&mut state, slot1.x, slot1.y, area);
        assert_eq!(state.placed[0].get(0).and_then(|gate| *gate), Some(Gate::H));
    }

    #[test]
    fn drop_between_adjacent_gates_inserts_column() {
        let area = Rect::new(0, 0, 100, 12);
        let mut state = AppState::new(Gate::H);
        let layout = circuit_layout(area, qubit_count(&state));
        let slot0 = layout.slots[0][0];
        state.placed[0] = vec![Some(Gate::H), Some(Gate::X)];
        state.dragging = Some(DragState {
            gate: Gate::Z,
            origin: DragOrigin::Palette,
        });
        state.drag_pos = Some((slot0.x, slot0.y));
        let gap_x = slot0.x.saturating_add(GATE_BOX_WIDTH);
        let gap_y = slot0.y + 1;
        handle_mouse_up(&mut state, gap_x, gap_y, area);
        assert_eq!(state.placed[0].get(0).and_then(|gate| *gate), Some(Gate::H));
        assert_eq!(state.placed[0].get(1).and_then(|gate| *gate), Some(Gate::Z));
        assert_eq!(state.placed[0].get(2).and_then(|gate| *gate), Some(Gate::X));
    }

    #[test]
    fn insert_snap_preferred_over_existing_gate() {
        let area = Rect::new(0, 0, 100, 12);
        let mut state = AppState::new(Gate::H);
        let layout = circuit_layout(area, qubit_count(&state));
        let slot0 = layout.slots[0][0];
        state.placed[0] = vec![Some(Gate::H), Some(Gate::X)];
        state.dragging = Some(DragState {
            gate: Gate::Z,
            origin: DragOrigin::Palette,
        });
        let gap_x = slot0.x.saturating_add(GATE_BOX_WIDTH);
        let gap_y = slot0.y + 1;
        update_hovered_slot(&mut state, gap_x, gap_y, area);
        assert_eq!(state.hovered_insert, Some((0, 1)));
        assert_eq!(state.hovered_slot, None);
    }

    #[test]
    fn drag_from_palette_to_circuit_sets_gate() {
        let area = Rect::new(0, 0, 60, 10);
        let mut state = AppState::new(Gate::H);
        let regions = layout_regions(area, qubit_count(&state));
        let items = palette_items(regions.palette);
        let target = items.iter().find(|item| item.gate == Gate::Z).unwrap();
        handle_mouse_down(&mut state, target.rect.x, target.rect.y, area);
        let layout = circuit_layout(area, qubit_count(&state));
        let slot = layout.slots[0][0];
        handle_mouse_up(&mut state, slot.x, slot.y, area);
        assert_eq!(state.placed[0][0], Some(Gate::Z));
    }

    #[test]
    fn drag_from_circuit_to_empty_removes_gate() {
        let area = Rect::new(0, 0, 60, 10);
        let mut state = AppState::new(Gate::H);
        let layout = circuit_layout(area, qubit_count(&state));
        let slot = layout.slots[0][0];
        handle_mouse_down(&mut state, slot.x, slot.y, area);
        handle_mouse_up(&mut state, area.x, area.y + 8, area);
        assert!(state.placed[0].is_empty());
    }

    #[test]
    fn dragging_from_circuit_hides_gate_until_drop() {
        let area = Rect::new(0, 0, 60, 10);
        let mut state = AppState::new(Gate::H);
        let layout = circuit_layout(area, qubit_count(&state));
        let slot = layout.slots[0][0];
        handle_mouse_down(&mut state, slot.x, slot.y, area);
        assert_eq!(state.placed[0][0], None);
        handle_mouse_up(&mut state, slot.x, slot.y, area);
        assert_eq!(state.placed[0][0], Some(Gate::H));
    }

    fn buffer_to_line(buffer: &Buffer, area: Rect, y: u16) -> String {
        let mut line = String::new();
        for x in 0..area.width {
            let cell = buffer.get(area.x + x, area.y + y);
            line.push_str(cell.symbol());
        }
        line.trim_end().to_string()
    }

    fn buffer_to_string(buffer: &Buffer, x: u16, y: u16, len: u16) -> String {
        let mut line = String::new();
        for offset in 0..len {
            let cell = buffer.get(x + offset, y);
            line.push_str(cell.symbol());
        }
        line
    }
}
