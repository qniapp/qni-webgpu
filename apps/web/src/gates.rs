#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GateKind {
    H,
    Control,
    AntiControl,
    BlochDisplay,
    Measurement,
    /// Quirk-compatible probability display. It has no unitary effect;
    /// a GPU compute pass marginalizes the live state vector into outcome
    /// bars and a render shader paints them without CPU readback.
    ProbabilityDisplay,
    /// Quirk-compatible amplitude display. It captures a span-local complex
    /// amplitude matrix into GPU storage and renders it without CPU readback.
    AmplitudeDisplay,
    /// Quirk-compatible density matrix display. It captures a span-local
    /// reduced density matrix into GPU storage and renders it without CPU readback.
    DensityMatrixDisplay,
    Spacer,
    Write0,
    Write1,
    X,
    Y,
    Z,
    SqrtX,
    S,
    SDagger,
    T,
    TDagger,
    Phase,
    Rx,
    Ry,
    Rz,
    Swap,
    /// Quantum Fourier Transform — multi-qubit gate whose `span` (number
    /// of qubits covered) is user-resizable via a hover-revealed handle.
    /// Simulation is expanded into a textbook GPU op stream by
    /// `simulation_plan::linearize_ops`; no CPU fallback is used.
    QftGate,
    /// Inverse QFT.
    QftDaggerGate,
}

mod info;
pub(crate) use info::{Amp, GateInfo};
mod params;
pub(crate) use params::{
    controlled_phase_params, gate_params, gate_params_controlled, parse_angle_radians,
    phase_params, rx_params, ry_params, rz_params, GateParams,
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct GateSpec {
    pub(crate) kind: GateKind,
    pub(crate) label: &'static str,
    pub(crate) url_token: &'static str,
    pub(crate) resizable_span: bool,
}

const GATE_SPECS: [GateSpec; 26] = [
    GateSpec {
        kind: GateKind::H,
        label: "H",
        url_token: "H",
        resizable_span: false,
    },
    GateSpec {
        kind: GateKind::Control,
        label: "C",
        url_token: "•",
        resizable_span: false,
    },
    GateSpec {
        kind: GateKind::AntiControl,
        label: "◦",
        url_token: "◦",
        resizable_span: false,
    },
    GateSpec {
        kind: GateKind::BlochDisplay,
        label: "B",
        url_token: "Bloch",
        resizable_span: false,
    },
    GateSpec {
        kind: GateKind::Measurement,
        label: "M",
        url_token: "Measure",
        resizable_span: false,
    },
    GateSpec {
        kind: GateKind::ProbabilityDisplay,
        label: "Probability",
        url_token: "Probability",
        resizable_span: true,
    },
    GateSpec {
        kind: GateKind::AmplitudeDisplay,
        label: "Amps",
        url_token: "Amps",
        resizable_span: true,
    },
    GateSpec {
        kind: GateKind::DensityMatrixDisplay,
        label: "Density",
        url_token: "Density",
        resizable_span: true,
    },
    GateSpec {
        kind: GateKind::Spacer,
        label: "…",
        url_token: "…",
        resizable_span: false,
    },
    GateSpec {
        kind: GateKind::Write0,
        label: "|0⟩",
        url_token: "|0>",
        resizable_span: false,
    },
    GateSpec {
        kind: GateKind::Write1,
        label: "|1⟩",
        url_token: "|1>",
        resizable_span: false,
    },
    GateSpec {
        kind: GateKind::X,
        label: "X",
        url_token: "X",
        resizable_span: false,
    },
    GateSpec {
        kind: GateKind::Y,
        label: "Y",
        url_token: "Y",
        resizable_span: false,
    },
    GateSpec {
        kind: GateKind::Z,
        label: "Z",
        url_token: "Z",
        resizable_span: false,
    },
    GateSpec {
        kind: GateKind::SqrtX,
        label: "√X",
        url_token: "X^½",
        resizable_span: false,
    },
    GateSpec {
        kind: GateKind::S,
        label: "S",
        url_token: "S",
        resizable_span: false,
    },
    GateSpec {
        kind: GateKind::SDagger,
        label: "S†",
        url_token: "S†",
        resizable_span: false,
    },
    GateSpec {
        kind: GateKind::T,
        label: "T",
        url_token: "T",
        resizable_span: false,
    },
    GateSpec {
        kind: GateKind::TDagger,
        label: "T†",
        url_token: "T†",
        resizable_span: false,
    },
    GateSpec {
        kind: GateKind::Phase,
        label: "Φ",
        url_token: "P",
        resizable_span: false,
    },
    GateSpec {
        kind: GateKind::Rx,
        label: "Rx",
        url_token: "Rx",
        resizable_span: false,
    },
    GateSpec {
        kind: GateKind::Ry,
        label: "Ry",
        url_token: "Ry",
        resizable_span: false,
    },
    GateSpec {
        kind: GateKind::Rz,
        label: "Rz",
        url_token: "Rz",
        resizable_span: false,
    },
    GateSpec {
        kind: GateKind::Swap,
        label: "SWAP",
        url_token: "Swap",
        resizable_span: false,
    },
    GateSpec {
        kind: GateKind::QftGate,
        label: "QFT",
        url_token: "QFT",
        resizable_span: true,
    },
    GateSpec {
        kind: GateKind::QftDaggerGate,
        label: "QFT†",
        url_token: "QFT†",
        resizable_span: true,
    },
];

// Split palette layout. The left Gates section keeps draggable operations and
// placeholders; the right Display section is a 2×2 grid for visualization
// widgets. Flat indices intentionally keep the pre-split IDs so hover, drag,
// tooltip tests, and serialized UI helpers can keep addressing the same gate.
pub(crate) const PALETTE_GATES_ROW1: [GateKind; 13] = [
    GateKind::H,
    GateKind::X,
    GateKind::Y,
    GateKind::Z,
    GateKind::SqrtX,
    GateKind::S,
    GateKind::SDagger,
    GateKind::T,
    GateKind::TDagger,
    GateKind::Phase,
    GateKind::Rx,
    GateKind::Ry,
    GateKind::Rz,
];
pub(crate) const PALETTE_GATES_ROW2: [GateKind; 9] = [
    GateKind::Swap,
    GateKind::Control,
    GateKind::AntiControl,
    GateKind::Write0,
    GateKind::Write1,
    GateKind::Measurement,
    GateKind::QftGate,
    GateKind::QftDaggerGate,
    GateKind::Spacer,
];
pub(crate) const PALETTE_DISPLAY_GATES: [GateKind; 4] = [
    GateKind::BlochDisplay,
    GateKind::ProbabilityDisplay,
    GateKind::AmplitudeDisplay,
    GateKind::DensityMatrixDisplay,
];
pub(crate) const PALETTE_GATE_COUNT: usize =
    PALETTE_GATES_ROW1.len() + PALETTE_GATES_ROW2.len() + PALETTE_DISPLAY_GATES.len();
pub(crate) const PALETTE_ROW1_COUNT: usize = PALETTE_GATES_ROW1.len();
pub(crate) const PALETTE_GATES_ROW2_INDICES: [usize; 9] = [13, 14, 15, 17, 18, 19, 22, 23, 21];
pub(crate) const PALETTE_DISPLAY_INDICES: [usize; 4] = [16, 20, 24, 25];

pub(crate) fn palette_gate_kind(index: usize) -> Option<GateKind> {
    if index < PALETTE_ROW1_COUNT {
        return Some(PALETTE_GATES_ROW1[index]);
    }
    if let Some(pos) = PALETTE_GATES_ROW2_INDICES
        .iter()
        .position(|candidate| *candidate == index)
    {
        return Some(PALETTE_GATES_ROW2[pos]);
    }
    PALETTE_DISPLAY_INDICES
        .iter()
        .position(|candidate| *candidate == index)
        .map(|pos| PALETTE_DISPLAY_GATES[pos])
}

impl GateKind {
    pub(crate) fn spec(self) -> &'static GateSpec {
        GATE_SPECS
            .iter()
            .find(|spec| spec.kind == self)
            .expect("GateKind must have a GateSpec")
    }

    pub(crate) fn from_url_token(token: &str) -> Option<Self> {
        GATE_SPECS
            .iter()
            .find(|spec| spec.url_token == token)
            .map(|spec| spec.kind)
    }

    pub(crate) fn label(self) -> &'static str {
        self.spec().label
    }

    /// Is this a multi-qubit gate whose vertical span is user-controlled
    /// via hover-revealed resize handles?
    pub(crate) fn is_resizable_span(self) -> bool {
        self.spec().resizable_span
    }

    /// Maximum user-resizable span for a gate in the current execution
    /// capacity. Probability mirrors Quirk's `Probability`/`Probability2..16` family even
    /// when the editor is in external-GPU mode; QFT follows the circuit
    /// capacity.
    pub(crate) fn max_resizable_span(self, qubit_capacity: usize) -> usize {
        match self {
            GateKind::ProbabilityDisplay | GateKind::AmplitudeDisplay => qubit_capacity.min(16),
            GateKind::DensityMatrixDisplay => qubit_capacity.min(8),
            _ => qubit_capacity,
        }
        .max(1)
    }
}
