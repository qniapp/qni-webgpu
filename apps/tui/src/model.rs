use crate::MIN_QUBIT_COUNT;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Gate {
    X,
    H,
    Y,
    Z,
    SqrtX,
    S,
    Sdg,
    T,
    Tdg,
    Swap,
}

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
    pub hovered_column: Option<(usize, usize)>,
    pub hovered_start: bool,
    pub confirmed_column: Option<usize>,
    pub confirmed_start: bool,
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
            hovered_column: None,
            hovered_start: false,
            confirmed_column: None,
            confirmed_start: false,
        }
    }
}

pub(crate) fn qubit_count(state: &AppState) -> usize {
    state.placed.len().max(MIN_QUBIT_COUNT)
}

pub(crate) fn ensure_slots(state: &mut AppState, counts: &[usize]) {
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

impl std::str::FromStr for Gate {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_uppercase().as_str() {
            "X" => Ok(Self::X),
            "H" => Ok(Self::H),
            "Y" => Ok(Self::Y),
            "Z" => Ok(Self::Z),
            "SQRTX" | "SX" | "√X" => Ok(Self::SqrtX),
            "S" => Ok(Self::S),
            "S†" | "SDG" | "S_DAGGER" => Ok(Self::Sdg),
            "T" => Ok(Self::T),
            "T†" | "TDG" | "T_DAGGER" => Ok(Self::Tdg),
            "SWAP" | "SW" => Ok(Self::Swap),
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
            Self::SqrtX => "√X",
            Self::S => "S",
            Self::Sdg => "S†",
            Self::T => "T",
            Self::Tdg => "T†",
            Self::Swap => "",
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
const HALF: f64 = 0.5;

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
        Gate::SqrtX => [
            Complex { re: HALF, im: HALF },
            Complex { re: HALF, im: -HALF },
            Complex { re: HALF, im: -HALF },
            Complex { re: HALF, im: HALF },
        ],
        Gate::S => [
            Complex { re: 1.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 0.0, im: 1.0 },
        ],
        Gate::Sdg => [
            Complex { re: 1.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 0.0, im: -1.0 },
        ],
        Gate::T => [
            Complex { re: 1.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            PHASE_45,
        ],
        Gate::Tdg => [
            Complex { re: 1.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex {
                re: INV_SQRT2,
                im: -INV_SQRT2,
            },
        ],
        Gate::Swap => [
            Complex { re: 1.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 1.0, im: 0.0 },
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
    if gate == Gate::Swap {
        return state;
    }
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

const ZERO: Complex = Complex { re: 0.0, im: 0.0 };
const ONE: Complex = Complex { re: 1.0, im: 0.0 };
const SWAP_MATRIX: [Complex; 16] = [
    ONE, ZERO, ZERO, ZERO,
    ZERO, ZERO, ONE, ZERO,
    ZERO, ONE, ZERO, ZERO,
    ZERO, ZERO, ZERO, ONE,
];

fn apply_two_qubit_matrix(state: [Complex; 4], matrix: [Complex; 16]) -> [Complex; 4] {
    let mut out = [ZERO; 4];
    for (row, out_slot) in out.iter_mut().enumerate() {
        let mut acc = ZERO;
        for col in 0..4 {
            let m = matrix[row * 4 + col];
            acc = add(acc, mul(m, state[col]));
        }
        *out_slot = acc;
    }
    out
}

pub fn apply_gates_to_zero(gates: &[Vec<Option<Gate>>]) -> [Complex; 4] {
    apply_gates_to_zero_limit(gates, None)
}

pub(crate) fn apply_gates_to_zero_limit(
    gates: &[Vec<Option<Gate>>],
    max_columns: Option<usize>,
) -> [Complex; 4] {
    let mut state = [
        Complex { re: 1.0, im: 0.0 },
        Complex { re: 0.0, im: 0.0 },
        Complex { re: 0.0, im: 0.0 },
        Complex { re: 0.0, im: 0.0 },
    ];
    let max_slots = gates.iter().map(|row| row.len()).max().unwrap_or(0);
    let limit = max_columns.unwrap_or(max_slots).min(max_slots);
    for slot in 0..limit {
        let has_swap_pair = gates
            .get(0)
            .and_then(|row| row.get(slot))
            .and_then(|gate| *gate)
            == Some(Gate::Swap)
            && gates
                .get(1)
                .and_then(|row| row.get(slot))
                .and_then(|gate| *gate)
                == Some(Gate::Swap);
        if has_swap_pair {
            state = apply_two_qubit_matrix(state, SWAP_MATRIX);
            continue;
        }
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
    build_state_line_with_limit(gates, None)
}

pub(crate) fn build_state_line_with_limit(
    gates: &[Vec<Option<Gate>>],
    max_columns: Option<usize>,
) -> String {
    let [amp0, amp1, amp2, amp3] = apply_gates_to_zero_limit(gates, max_columns);
    format!(
        "State: [({}), ({}), ({}), ({})]",
        format_complex(amp0),
        format_complex(amp1),
        format_complex(amp2),
        format_complex(amp3)
    )
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
