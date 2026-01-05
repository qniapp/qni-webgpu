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
const PALETTE_LABEL: &str = "Palette: ";
const GATE_BOX_WIDTH: u16 = 5;
const GATE_BOX_HEIGHT: u16 = 3;
const PALETTE_HEIGHT: u16 = GATE_BOX_HEIGHT;
const PALETTE_GAP: u16 = 2;
const CIRCUIT_OFFSET: u16 = PALETTE_HEIGHT + PALETTE_GAP;
const GATE_GAP: u16 = 1;

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
    pub placed: Option<Gate>,
    pub dragging: Option<DragState>,
    pub drag_pos: Option<(u16, u16)>,
}

impl AppState {
    pub fn new(initial_gate: Gate) -> Self {
        Self {
            placed: Some(initial_gate),
            dragging: None,
            drag_pos: None,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PaletteItem {
    pub gate: Gate,
    pub rect: Rect,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Regions {
    pub palette: Rect,
    pub circuit: Rect,
    pub state: Rect,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct DragVisual {
    pub gate: Gate,
    pub x: u16,
    pub y: u16,
}

pub fn layout_regions(area: Rect) -> Regions {
    let state_y = area
        .y
        .saturating_add(area.height.saturating_sub(1));
    let palette_bottom = area.y.saturating_add(PALETTE_HEIGHT);
    let desired_circuit_y = area.y.saturating_add(CIRCUIT_OFFSET);
    let max_circuit_y = state_y.saturating_sub(GATE_BOX_HEIGHT + 1);
    let circuit_y = if max_circuit_y < palette_bottom {
        palette_bottom
    } else {
        desired_circuit_y.min(max_circuit_y)
    };
    Regions {
        palette: Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: PALETTE_HEIGHT,
        },
        circuit: Rect {
            x: area.x,
            y: circuit_y,
            width: area.width,
            height: 1,
        },
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
    let label_width = PALETTE_LABEL.chars().count() as u16;
    let mut x = area.x + label_width + 1;
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
    let shadow = Color::DarkGray;
    (text, background, highlight, shadow)
}

fn draw_gate_box(buffer: &mut Buffer, rect: Rect, gate: Gate) {
    if rect.width < GATE_BOX_WIDTH || rect.height < GATE_BOX_HEIGHT {
        return;
    }
    let (text, background, highlight, shadow) = gate_theme(gate);
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
            Style::default().fg(highlight).bg(background),
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
    if rect.width > 2 && rect.height > 1 {
        for row in 0..rect.height {
            let y = rect.y + row;
            buffer.set_string(
                rect.x,
                y,
                "▏",
                Style::default().fg(highlight).bg(background),
            );
            buffer.set_string(
                rect.x + rect.width - 1,
                y,
                "▕",
                Style::default().fg(shadow).bg(background),
            );
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

pub fn hit_test_palette(x: u16, y: u16, area: Rect) -> Option<Gate> {
    let regions = layout_regions(area);
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

fn circuit_gate_range(area: Rect) -> std::ops::Range<u16> {
    let prefix_len = "q0: ---".len() as u16;
    let start = area.x + prefix_len;
    let end = start + GATE_BOX_WIDTH;
    start..end
}

pub fn hit_test_circuit_slot(x: u16, y: u16, area: Rect) -> bool {
    let regions = layout_regions(area);
    let range = circuit_gate_range(regions.circuit);
    let rect = Rect {
        x: range.start,
        y: regions.circuit.y,
        width: GATE_BOX_WIDTH,
        height: GATE_BOX_HEIGHT,
    };
    x >= rect.x
        && x < rect.x.saturating_add(rect.width)
        && y >= rect.y
        && y < rect.y.saturating_add(rect.height)
}

pub fn is_empty_drop(_x: u16, y: u16, area: Rect) -> bool {
    let regions = layout_regions(area);
    let palette_bottom = regions.palette.y.saturating_add(regions.palette.height);
    let circuit_top = regions.circuit.y;
    let circuit_bottom = circuit_top.saturating_add(GATE_BOX_HEIGHT);
    let within_palette = y >= regions.palette.y && y < palette_bottom;
    let within_circuit = y >= circuit_top && y < circuit_bottom;
    !within_palette && !within_circuit && y != regions.state.y
}

pub fn handle_mouse_down(state: &mut AppState, x: u16, y: u16, area: Rect) {
    if let Some(gate) = hit_test_palette(x, y, area) {
        state.dragging = Some(DragState {
            gate,
            origin: DragOrigin::Palette,
        });
        state.drag_pos = Some((x, y));
        return;
    }

    if hit_test_circuit_slot(x, y, area) {
        if let Some(gate) = state.placed {
            state.dragging = Some(DragState {
                gate,
                origin: DragOrigin::Circuit,
            });
            state.drag_pos = Some((x, y));
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

    if hit_test_circuit_slot(x, y, area) {
        state.placed = Some(dragging.gate);
    } else if is_empty_drop(x, y, area) && dragging.origin == DragOrigin::Circuit {
        state.placed = None;
    }

    state.dragging = None;
    state.drag_pos = None;
}

pub fn displayed_gate(state: &AppState) -> Option<Gate> {
    match state.dragging {
        Some(drag) if drag.origin == DragOrigin::Circuit => None,
        _ => state.placed,
    }
}

impl Gate {
    pub fn from_str(value: &str) -> Option<Self> {
        match value.trim().to_uppercase().as_str() {
            "X" => Some(Self::X),
            "H" => Some(Self::H),
            "Y" => Some(Self::Y),
            "Z" => Some(Self::Z),
            "S" => Some(Self::S),
            "T" => Some(Self::T),
            _ => None,
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
            Complex { re: INV_SQRT2, im: 0.0 },
            Complex { re: INV_SQRT2, im: 0.0 },
            Complex { re: INV_SQRT2, im: 0.0 },
            Complex { re: -INV_SQRT2, im: 0.0 },
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

pub fn apply_gate_to_zero(gate: Gate) -> [Complex; 2] {
    let zero = Complex { re: 1.0, im: 0.0 };
    let one = Complex { re: 0.0, im: 0.0 };
    let [a00, a01, a10, a11] = matrix_for(gate);

    let out0 = add(mul(a00, zero), mul(a01, one));
    let out1 = add(mul(a10, zero), mul(a11, one));
    [out0, out1]
}

pub fn apply_gate_to_zero_optional(gate: Option<Gate>) -> [Complex; 2] {
    match gate {
        Some(gate) => apply_gate_to_zero(gate),
        None => [
            Complex { re: 1.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
        ],
    }
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

pub fn build_lines(gate: Option<Gate>, debug_line: Option<&str>) -> Vec<String> {
    let inner = " ".repeat(GATE_BOX_WIDTH as usize);
    let wire = format!("---{}---", inner);
    let circuit_line = format!("q0: {}", wire);
    let [amp0, amp1] = apply_gate_to_zero_optional(gate);
    let state_line = format!(
        "State: [({}), ({})]",
        format_complex(amp0),
        format_complex(amp1)
    );
    let mut lines = vec![circuit_line];
    if let Some(debug) = debug_line {
        lines.push(format!("Debug: {}", debug));
    }
    lines.push(state_line);
    lines
}

pub fn render_to_buffer(gate: Option<Gate>, area: Rect, debug_line: Option<&str>) -> Buffer {
    render_to_buffer_with_drag(gate, area, debug_line, None)
}

pub fn render_to_buffer_with_drag(
    gate: Option<Gate>,
    area: Rect,
    debug_line: Option<&str>,
    drag: Option<DragVisual>,
) -> Buffer {
    let mut buffer = Buffer::empty(area);
    let regions = layout_regions(area);
    let lines = build_lines(gate, None);
    buffer.set_string(
        regions.palette.x,
        regions.palette.y,
        PALETTE_LABEL,
        Style::default(),
    );
    for item in palette_items(regions.palette) {
        draw_gate_box(&mut buffer, item.rect, item.gate);
    }

    if let Some(circuit) = lines.get(0) {
        buffer.set_string(
            regions.circuit.x,
            regions.circuit.y,
            circuit,
            Style::default(),
        );
    }
    if let Some(gate) = gate {
        let gate_range = circuit_gate_range(regions.circuit);
        let rect = Rect {
            x: gate_range.start,
            y: regions.circuit.y,
            width: GATE_BOX_WIDTH,
            height: GATE_BOX_HEIGHT,
        };
        draw_gate_box(&mut buffer, rect, gate);
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
    if let Some(state) = lines.last() {
        let text = Text::from(state.clone());
        let paragraph = Paragraph::new(text);
        paragraph.render(regions.state, &mut buffer);
    }
    if let Some(drag) = drag {
        if drag.x >= area.x
            && drag.y >= area.y
            && drag.x < area.x.saturating_add(area.width)
            && drag.y < area.y.saturating_add(area.height)
        {
            let rect = Rect {
                x: drag.x,
                y: drag.y,
                width: GATE_BOX_WIDTH,
                height: GATE_BOX_HEIGHT,
            };
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
        .and_then(Gate::from_str)
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
    fn build_lines_defaults_to_h_gate() {
        let lines = build_lines(Some(Gate::H), None);
        assert_eq!(
            lines,
            vec![
                "q0: ---     ---".to_string(),
                "State: [(0.7071067811865475+0i), (0.7071067811865475+0i)]".to_string(),
            ]
        );
    }

    #[test]
    fn build_lines_for_y_gate() {
        let lines = build_lines(Some(Gate::Y), None);
        assert_eq!(
            lines,
            vec![
                "q0: ---     ---".to_string(),
                "State: [(0+0i), (0+1i)]".to_string(),
            ]
        );
    }

    #[test]
    fn build_lines_with_no_gate() {
        let lines = build_lines(None, None);
        assert_eq!(
            lines,
            vec![
                "q0: ---     ---".to_string(),
                "State: [(1+0i), (0+0i)]".to_string(),
            ]
        );
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
        let area = Rect::new(0, 0, 60, 8);
        let buffer = render_to_buffer(Some(Gate::H), area, None);
        let line0 = buffer_to_line(&buffer, area, 0);
        let line4 = buffer_to_line(&buffer, area, 4);
        let line7 = buffer_to_line(&buffer, area, 7);
        assert!(line0.starts_with("Palette: "));
        assert_eq!(line4, "q0: ---▏ H ▕---");
        assert_eq!(
            line7,
            "State: [(0.7071067811865475+0i), (0.7071067811865475+0i)]"
        );
    }

    #[test]
    fn render_to_buffer_shows_drag_overlay() {
        let area = Rect::new(0, 0, 20, 8);
        let drag = DragVisual {
            gate: Gate::X,
            x: 5,
            y: 3,
        };
        let buffer = render_to_buffer_with_drag(Some(Gate::H), area, None, Some(drag));
        let overlay = buffer_to_string(&buffer, 5, 3, 5);
        assert_eq!(overlay, "▏▔▔▔▕");
    }

    #[test]
    fn hit_test_palette_finds_gate() {
        let area = Rect::new(0, 0, 60, 6);
        let regions = layout_regions(area);
        let items = palette_items(regions.palette);
        let target = items.iter().find(|item| item.gate == Gate::X).unwrap();
        let x = target.rect.x;
        let gate = hit_test_palette(x, target.rect.y, area);
        assert_eq!(gate, Some(Gate::X));
    }

    #[test]
    fn drag_from_palette_to_circuit_sets_gate() {
        let area = Rect::new(0, 0, 60, 10);
        let mut state = AppState::new(Gate::H);
        let regions = layout_regions(area);
        let items = palette_items(regions.palette);
        let target = items.iter().find(|item| item.gate == Gate::Z).unwrap();
        handle_mouse_down(&mut state, target.rect.x, target.rect.y, area);
        let gate_slot = circuit_gate_range(regions.circuit);
        handle_mouse_up(&mut state, gate_slot.start, regions.circuit.y, area);
        assert_eq!(state.placed, Some(Gate::Z));
    }

    #[test]
    fn drag_from_circuit_to_empty_removes_gate() {
        let area = Rect::new(0, 0, 60, 10);
        let mut state = AppState::new(Gate::H);
        let regions = layout_regions(area);
        let gate_slot = circuit_gate_range(regions.circuit);
        handle_mouse_down(&mut state, gate_slot.start, regions.circuit.y, area);
        handle_mouse_up(&mut state, area.x, area.y + 8, area);
        assert_eq!(state.placed, None);
    }

    #[test]
    fn dragging_from_circuit_hides_gate_until_drop() {
        let area = Rect::new(0, 0, 60, 10);
        let mut state = AppState::new(Gate::H);
        let regions = layout_regions(area);
        let gate_slot = circuit_gate_range(regions.circuit);
        handle_mouse_down(&mut state, gate_slot.start, regions.circuit.y, area);
        assert_eq!(displayed_gate(&state), None);
        handle_mouse_up(&mut state, gate_slot.start, regions.circuit.y, area);
        assert_eq!(displayed_gate(&state), Some(Gate::H));
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
