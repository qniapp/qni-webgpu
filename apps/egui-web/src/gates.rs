#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GateKind {
    H,
    Control,
    AntiControl,
    BlochDisplay,
    Measurement,
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
pub(crate) use info::Amp;
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

const GATE_SPECS: [GateSpec; 23] = [
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
        label: "P",
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

// Two-row palette layout. Row 1 holds unitary single-qubit gates; row 2 holds
// special-purpose gates. Indices remain flat so callers can look up a gate
// without branching.
pub(crate) const PALETTE_ROW1_COUNT: usize = 13;
pub(crate) const PALETTE_GATES: [GateKind; 23] = [
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
    GateKind::Swap,
    GateKind::Control,
    GateKind::AntiControl,
    GateKind::BlochDisplay,
    GateKind::Write0,
    GateKind::Write1,
    GateKind::Measurement,
    GateKind::Spacer,
    GateKind::QftGate,
    GateKind::QftDaggerGate,
];

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
    /// via the hover-revealed resize handle? Right now QFT / QFT† are the
    /// only such gates.
    pub(crate) fn is_resizable_span(self) -> bool {
        self.spec().resizable_span
    }
}
