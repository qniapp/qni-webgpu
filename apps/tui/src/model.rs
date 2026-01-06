use crate::MIN_QUBIT_COUNT;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Gate {
    X,
    Control,
    Measure,
    Phase,
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
pub struct PhaseValue {
    pub phi: f64,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct PhaseEdit {
    pub row: usize,
    pub slot: usize,
    pub input: String,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub placed: Vec<Vec<Option<Gate>>>,
    pub phase_values: Vec<Vec<Option<PhaseValue>>>,
    pub dragging: Option<DragState>,
    pub drag_pos: Option<(u16, u16)>,
    pub drag_phase_value: Option<PhaseValue>,
    pub hovered_slot: Option<usize>,
    pub hovered_row: Option<usize>,
    pub hovered_insert: Option<(usize, usize)>,
    pub hovered_column: Option<(usize, usize)>,
    pub hovered_start: bool,
    pub confirmed_column: Option<usize>,
    pub confirmed_start: bool,
    pub phase_edit: Option<PhaseEdit>,
    pub phase_edit_error: Option<String>,
    pub cache_valid: bool,
    pub cached_state_line: String,
    pub cached_measurements: Vec<Vec<Option<u8>>>,
    pub cached_limit: Option<usize>,
    pub cached_full_measurements: Vec<Vec<Option<u8>>>,
    pub cached_full_valid: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            placed: vec![Vec::new(); MIN_QUBIT_COUNT],
            phase_values: vec![Vec::new(); MIN_QUBIT_COUNT],
            dragging: None,
            drag_pos: None,
            drag_phase_value: None,
            hovered_slot: None,
            hovered_row: None,
            hovered_insert: None,
            hovered_column: None,
            hovered_start: false,
            confirmed_column: None,
            confirmed_start: false,
            phase_edit: None,
            phase_edit_error: None,
            cache_valid: false,
            cached_state_line: String::new(),
            cached_measurements: Vec::new(),
            cached_limit: None,
            cached_full_measurements: Vec::new(),
            cached_full_valid: false,
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
            state.phase_values.push(Vec::new());
        }
        state.placed[row].resize(count, None);
        state.phase_values[row].resize(count, None);
    }
}

impl std::str::FromStr for Gate {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_uppercase().as_str() {
            "X" => Ok(Self::X),
            "CTRL" | "CONTROL" | "C" => Ok(Self::Control),
            "MEASURE" | "MEAS" | "M" => Ok(Self::Measure),
            "PHASE" | "PHI" | "PHASEGATE" => Ok(Self::Phase),
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
            Self::Control => "",
            Self::Measure => "",
            Self::Phase => "Φ",
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
        Gate::Phase => [
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
        Gate::Swap | Gate::Control | Gate::Measure => [
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

pub fn apply_gate_to_state(
    state: &[Complex],
    gate: Gate,
    target: usize,
    qubits: usize,
) -> Vec<Complex> {
    if gate == Gate::Swap || gate == Gate::Control || gate == Gate::Measure {
        return state.to_vec();
    }
    if target >= qubits {
        return state.to_vec();
    }
    let [a00, a01, a10, a11] = matrix_for(gate);
    let mut out = state.to_vec();
    let bit = qubits.saturating_sub(1).saturating_sub(target);
    let mask = 1usize << bit;
    for i in 0..state.len() {
        if i & mask == 0 {
            let i0 = i;
            let i1 = i | mask;
            let v0 = state[i0];
            let v1 = state[i1];
            out[i0] = add(mul(a00, v0), mul(a01, v1));
            out[i1] = add(mul(a10, v0), mul(a11, v1));
        }
    }
    out
}

const ZERO: Complex = Complex { re: 0.0, im: 0.0 };
const ONE: Complex = Complex { re: 1.0, im: 0.0 };

fn apply_swap_qubits(state: &[Complex], q0: usize, q1: usize, qubits: usize) -> Vec<Complex> {
    if q0 >= qubits || q1 >= qubits || q0 == q1 {
        return state.to_vec();
    }
    let bit0 = qubits.saturating_sub(1).saturating_sub(q0);
    let bit1 = qubits.saturating_sub(1).saturating_sub(q1);
    let mask0 = 1usize << bit0;
    let mask1 = 1usize << bit1;
    let mut out = vec![ZERO; state.len()];
    for (index, value) in state.iter().enumerate() {
        let b0 = index & mask0;
        let b1 = index & mask1;
        if (b0 == 0) == (b1 == 0) {
            out[index] = *value;
        } else {
            let swapped = index ^ mask0 ^ mask1;
            out[swapped] = *value;
        }
    }
    out
}

fn apply_cnot_qubits(state: &[Complex], control: usize, target: usize, qubits: usize) -> Vec<Complex> {
    if control >= qubits || target >= qubits || control == target {
        return state.to_vec();
    }
    let control_bit = qubits.saturating_sub(1).saturating_sub(control);
    let target_bit = qubits.saturating_sub(1).saturating_sub(target);
    let control_mask = 1usize << control_bit;
    let target_mask = 1usize << target_bit;
    let mut out = state.to_vec();
    for index in 0..state.len() {
        if index & control_mask != 0 && index & target_mask == 0 {
            let flipped = index | target_mask;
            out.swap(index, flipped);
        }
    }
    out
}

fn measure_qubit(state: &[Complex], qubit: usize, qubits: usize) -> (Vec<Complex>, u8) {
    if qubit >= qubits {
        return (state.to_vec(), 0);
    }
    let bit = qubits.saturating_sub(1).saturating_sub(qubit);
    let mask = 1usize << bit;
    let mut p0 = 0.0;
    for (index, amp) in state.iter().enumerate() {
        if index & mask == 0 {
            p0 += amp.re * amp.re + amp.im * amp.im;
        }
    }
    let rand = random_unit_f64();
    let outcome = if rand <= p0 { 0 } else { 1 };
    let prob = if outcome == 0 { p0 } else { 1.0 - p0 };
    let scale = if prob > 0.0 { 1.0 / prob.sqrt() } else { 0.0 };
    let mut collapsed = vec![ZERO; state.len()];
    for (index, amp) in state.iter().enumerate() {
        let is_one = index & mask != 0;
        if (outcome == 1 && is_one) || (outcome == 0 && !is_one) {
            collapsed[index] = Complex {
                re: amp.re * scale,
                im: amp.im * scale,
            };
        }
    }
    (collapsed, outcome)
}

pub(crate) fn default_phase_value() -> PhaseValue {
    PhaseValue {
        phi: std::f64::consts::FRAC_PI_2,
        label: "π/2".to_string(),
    }
}

pub(crate) fn parse_phase_label(input: &str) -> Result<PhaseValue, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("φ の値が空です".to_string());
    }
    let normalized = trimmed.replace("pi", "π").replace("PI", "π").replace(' ', "");
    let mut chars = normalized.chars().peekable();
    let mut sign = 1i32;
    if let Some(&ch) = chars.peek() {
        if ch == '-' {
            sign = -1;
            chars.next();
        } else if ch == '+' {
            chars.next();
        }
    }
    let mut coeff_str = String::new();
    while let Some(&ch) = chars.peek() {
        if ch.is_ascii_digit() {
            coeff_str.push(ch);
            chars.next();
        } else {
            break;
        }
    }
    let Some(pi_ch) = chars.next() else {
        return Err("π を含む形式で入力してください".to_string());
    };
    if pi_ch != 'π' {
        return Err("π を含む形式で入力してください".to_string());
    }
    let coeff = if coeff_str.is_empty() {
        1i32
    } else {
        coeff_str
            .parse::<i32>()
            .map_err(|_| "係数が不正です".to_string())?
    };
    if coeff == 0 {
        return Err("係数は 0 以外を指定してください".to_string());
    }
    let mut denom = 1i32;
    if let Some(ch) = chars.next() {
        if ch != '/' {
            return Err("分母は /n の形式で入力してください".to_string());
        }
        let rest: String = chars.collect();
        if rest.is_empty() {
            return Err("分母が空です".to_string());
        }
        denom = rest
            .parse::<i32>()
            .map_err(|_| "分母が不正です".to_string())?;
        if denom == 0 {
            return Err("分母は 0 以外を指定してください".to_string());
        }
    }
    let sign_prefix = if sign < 0 { "-" } else { "" };
    let coeff_value = coeff.abs();
    let coeff_label = if coeff_value == 1 {
        String::new()
    } else {
        coeff_value.to_string()
    };
    let label = if denom == 1 {
        format!("{}{}π", sign_prefix, coeff_label)
    } else {
        format!("{}{}π/{}", sign_prefix, coeff_label, denom)
    };
    let phi = (sign as f64) * (coeff.abs() as f64) * std::f64::consts::PI / (denom as f64);
    Ok(PhaseValue { phi, label })
}

fn random_unit_f64() -> f64 {
    static RNG_STATE: AtomicU64 = AtomicU64::new(0);
    let mut state = RNG_STATE.load(Ordering::Relaxed);
    if state == 0 {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        state = seed.max(1);
        RNG_STATE.store(state, Ordering::Relaxed);
    }
    loop {
        let mut next = state;
        next ^= next << 13;
        next ^= next >> 7;
        next ^= next << 17;
        if RNG_STATE
            .compare_exchange(state, next, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            return (next as f64) / (u64::MAX as f64);
        }
        state = RNG_STATE.load(Ordering::Relaxed);
    }
}

#[derive(Debug, Clone)]
pub struct SimulationResult {
    pub state: Vec<Complex>,
    pub measurements: Vec<Vec<Option<u8>>>,
}

pub fn apply_gates_to_zero(gates: &[Vec<Option<Gate>>]) -> Vec<Complex> {
    apply_gates_to_zero_limit(gates, None, None).state
}

pub(crate) fn apply_gates_to_zero_limit(
    gates: &[Vec<Option<Gate>>],
    max_columns: Option<usize>,
    phase_values: Option<&Vec<Vec<Option<PhaseValue>>>>,
) -> SimulationResult {
    let qubits = gates.len().max(MIN_QUBIT_COUNT);
    let mut state = vec![ZERO; 1usize << qubits];
    state[0] = ONE;
    let max_slots = gates.iter().map(|row| row.len()).max().unwrap_or(0);
    let limit = max_columns.unwrap_or(max_slots).min(max_slots);
    let mut measurements = vec![vec![None; max_slots]; qubits];
    for slot in 0..limit {
        let mut swap_rows = Vec::new();
        let mut control_row = None;
        let mut target_row = None;
        let mut measure_rows = Vec::new();
        for (row, row_gates) in gates.iter().enumerate() {
            let gate = row_gates.get(slot).and_then(|gate| *gate);
            match gate {
                Some(Gate::Swap) => swap_rows.push(row),
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
                Some(Gate::Measure) => measure_rows.push(row),
                _ => {}
            }
        }
        if swap_rows.len() == 2 {
            state = apply_swap_qubits(&state, swap_rows[0], swap_rows[1], qubits);
        }
        let mut cnot_target = None;
        if let (Some(control), Some(target)) = (control_row, target_row) {
            if control != target {
                state = apply_cnot_qubits(&state, control, target, qubits);
                cnot_target = Some(target);
            }
        }
        for (row, row_gates) in gates.iter().enumerate() {
            if let Some(Some(gate)) = row_gates.get(slot) {
                if *gate == Gate::Swap || *gate == Gate::Control || *gate == Gate::Measure {
                    continue;
                }
                if *gate == Gate::X && cnot_target == Some(row) {
                    continue;
                }
                if *gate == Gate::Phase {
                    let phi = phase_values
                        .and_then(|values| values.get(row))
                        .and_then(|row_values| row_values.get(slot))
                        .and_then(|value| value.as_ref())
                        .map(|value| value.phi)
                        .unwrap_or(default_phase_value().phi);
                    state = apply_phase_gate(&state, row, phi, qubits);
                } else {
                    state = apply_gate_to_state(&state, *gate, row, qubits);
                }
            }
        }
        for row in measure_rows {
            let (next_state, outcome) = measure_qubit(&state, row, qubits);
            state = next_state;
            if let Some(row_measurements) = measurements.get_mut(row) {
                if let Some(slot_value) = row_measurements.get_mut(slot) {
                    *slot_value = Some(outcome);
                }
            }
        }
    }
    SimulationResult { state, measurements }
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
    let amplitudes = apply_gates_to_zero_limit(gates, max_columns, None).state;
    let formatted: Vec<String> = amplitudes
        .iter()
        .map(|amp| format!("({})", format_complex(*amp)))
        .collect();
    format!("State: [{}]", formatted.join(", "))
}

fn apply_phase_gate(state: &[Complex], target: usize, phi: f64, qubits: usize) -> Vec<Complex> {
    if target >= qubits {
        return state.to_vec();
    }
    let bit = qubits.saturating_sub(1).saturating_sub(target);
    let mask = 1usize << bit;
    let phase = Complex {
        re: phi.cos(),
        im: phi.sin(),
    };
    let mut out = state.to_vec();
    for index in 0..state.len() {
        if index & mask != 0 {
            out[index] = mul(phase, state[index]);
        }
    }
    out
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
