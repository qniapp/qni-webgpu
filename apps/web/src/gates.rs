#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GateKind {
    H,
    Control,
    AntiControl,
    ZParityControl,
    XParityControl,
    SampleDisplay,
    BlochDisplay,
    Measurement,
    ZDetectControlReset,
    XDetector,
    YDetector,
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

mod angle;
pub(crate) use angle::ParametricAngle;
mod controls;
pub(crate) use controls::ColumnControls;
mod flag;
pub(crate) use flag::{FlagName, GateFlag};
mod span;
pub(crate) use span::GateSpan;
mod info;
pub(crate) use info::{Amp, GateInfo};
mod params;
#[cfg(test)]
pub(crate) use params::GATE_UNCONDITIONAL;
pub(crate) use params::{
    gate_params, gate_params_controlled, phase_params, rx_params, ry_params, rz_params, GateParams,
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct GateSpec {
    pub(crate) kind: GateKind,
    pub(crate) label: &'static str,
    pub(crate) url_token: &'static str,
    pub(crate) resizable_span: bool,
}

const GATE_SPECS: [GateSpec; 32] = [
    GateSpec {
        kind: GateKind::ZParityControl,
        label: "Zpar",
        url_token: "zpar",
        resizable_span: false,
    },
    GateSpec {
        kind: GateKind::XParityControl,
        label: "Xpar",
        url_token: "xpar",
        resizable_span: false,
    },
    GateSpec {
        kind: GateKind::SampleDisplay,
        label: "Sample",
        url_token: "Sample",
        resizable_span: true,
    },
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
        kind: GateKind::ZDetectControlReset,
        label: "ZDetectControlReset",
        url_token: "ZDetectControlReset",
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
    GateSpec {
        kind: GateKind::XDetector,
        label: "X Detector",
        url_token: "XDetector",
        resizable_span: false,
    },
    GateSpec {
        kind: GateKind::YDetector,
        label: "Y Detector",
        url_token: "YDetector",
        resizable_span: false,
    },
];

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
pub(crate) const PALETTE_GATES_ROW2: [GateKind; 14] = [
    GateKind::Swap,
    GateKind::Control,
    GateKind::AntiControl,
    GateKind::ZParityControl,
    GateKind::XParityControl,
    GateKind::Write0,
    GateKind::Write1,
    GateKind::Measurement,
    GateKind::ZDetectControlReset,
    GateKind::XDetector,
    GateKind::YDetector,
    GateKind::QftGate,
    GateKind::QftDaggerGate,
    GateKind::Spacer,
];
pub(crate) const PALETTE_DISPLAY_GATES: [GateKind; 5] = [
    GateKind::BlochDisplay,
    GateKind::ProbabilityDisplay,
    GateKind::SampleDisplay,
    GateKind::AmplitudeDisplay,
    GateKind::DensityMatrixDisplay,
];
pub(crate) const PALETTE_GATE_COUNT: usize =
    PALETTE_GATES_ROW1.len() + PALETTE_GATES_ROW2.len() + PALETTE_DISPLAY_GATES.len();
pub(crate) const PALETTE_ROW1_COUNT: usize = PALETTE_GATES_ROW1.len();
pub(crate) const PALETTE_GATES_ROW2_INDICES: [usize; 14] =
    [13, 14, 15, 27, 28, 17, 18, 19, 26, 30, 31, 22, 23, 21];
pub(crate) const PALETTE_DISPLAY_INDICES: [usize; 5] = [16, 20, 29, 24, 25];

pub(crate) fn default_palette_angle(kind: GateKind) -> Option<ParametricAngle> {
    match kind {
        GateKind::Phase | GateKind::Rx | GateKind::Ry | GateKind::Rz => {
            Some(ParametricAngle::default())
        }
        _ => None,
    }
}

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
    pub(crate) fn is_sampled_measurement(self) -> bool {
        matches!(self, Self::Measurement | Self::XDetector | Self::YDetector)
    }

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
            GateKind::SampleDisplay => qubit_capacity.min(3),
            _ => qubit_capacity,
        }
        .max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_info_explicitly_describes_no_collapse() {
        assert!(GateKind::SampleDisplay
            .info()
            .paragraphs
            .iter()
            .any(|text| text.contains("without changing or collapsing")));
    }

    #[test]
    fn sample_span_is_limited_to_three_wires() {
        assert_eq!(GateKind::SampleDisplay.max_resizable_span(16), 3);
    }

    #[test]
    fn new_palette_entries_have_stable_indices() {
        assert_eq!(
            [26, 27, 28, 29].map(palette_gate_kind),
            [
                Some(GateKind::ZDetectControlReset),
                Some(GateKind::ZParityControl),
                Some(GateKind::XParityControl),
                Some(GateKind::SampleDisplay),
            ]
        );
    }

    #[test]
    fn detectors_do_not_accept_measurement_flags() {
        let flag = GateFlag::Set(FlagName::parse("outcome").unwrap());
        assert_eq!(
            [
                GateKind::Measurement,
                GateKind::XDetector,
                GateKind::YDetector
            ]
            .map(|kind| flag.fits(kind)),
            [true, false, false]
        );
    }

    #[test]
    fn detector_palette_ids_follow_historical_gates() {
        assert_eq!(
            [
                palette_gate_kind(26),
                palette_gate_kind(30),
                palette_gate_kind(31),
            ],
            [
                Some(GateKind::ZDetectControlReset),
                Some(GateKind::XDetector),
                Some(GateKind::YDetector),
            ]
        );
    }

    #[test]
    fn sampled_measurements_include_both_detectors() {
        assert_eq!(
            [
                GateKind::Measurement,
                GateKind::XDetector,
                GateKind::YDetector,
                GateKind::H
            ]
            .map(GateKind::is_sampled_measurement),
            [true, true, true, false]
        );
    }

    #[test]
    fn phase_palette_default_angle_is_pi_over_two() {
        assert_eq!(
            default_palette_angle(GateKind::Phase).unwrap().label(),
            "π/2"
        );
    }

    #[test]
    fn rx_palette_default_angle_is_pi_over_two() {
        assert_eq!(default_palette_angle(GateKind::Rx).unwrap().label(), "π/2");
    }

    #[test]
    fn ry_palette_default_angle_is_pi_over_two() {
        assert_eq!(default_palette_angle(GateKind::Ry).unwrap().label(), "π/2");
    }

    #[test]
    fn rz_palette_default_angle_is_pi_over_two() {
        assert_eq!(default_palette_angle(GateKind::Rz).unwrap().label(), "π/2");
    }
}
