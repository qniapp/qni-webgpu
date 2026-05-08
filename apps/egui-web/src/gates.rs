#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GateKind {
    H,
    Control,
    AntiControl,
    BlochDisplay,
    Measurement,
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
}

impl GateKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            GateKind::H => "H",
            GateKind::Control => "C",
            GateKind::AntiControl => "◦",
            GateKind::BlochDisplay => "B",
            GateKind::Measurement => "M",
            GateKind::Write0 => "|0⟩",
            GateKind::Write1 => "|1⟩",
            GateKind::X => "X",
            GateKind::Y => "Y",
            GateKind::Z => "Z",
            GateKind::SqrtX => "√X",
            GateKind::S => "S",
            GateKind::SDagger => "S†",
            GateKind::T => "T",
            GateKind::TDagger => "T†",
            GateKind::Phase => "P",
            GateKind::Rx => "Rx",
            GateKind::Ry => "Ry",
            GateKind::Rz => "Rz",
            GateKind::Swap => "SWAP",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct GateMatrix {
    pub(crate) m00: [f32; 2],
    pub(crate) m01: [f32; 2],
    pub(crate) m10: [f32; 2],
    pub(crate) m11: [f32; 2],
}

pub(crate) fn gate_matrix(kind: GateKind) -> GateMatrix {
    let inv_sqrt2 = std::f32::consts::FRAC_1_SQRT_2;
    let default_angle = std::f32::consts::FRAC_PI_2;
    let half_angle = default_angle * 0.5;
    let cos_half = half_angle.cos();
    let sin_half = half_angle.sin();
    let exp_i = |angle: f32| [angle.cos(), angle.sin()];
    match kind {
        GateKind::H => GateMatrix {
            m00: [inv_sqrt2, 0.0],
            m01: [inv_sqrt2, 0.0],
            m10: [inv_sqrt2, 0.0],
            m11: [-inv_sqrt2, 0.0],
        },
        GateKind::Control | GateKind::AntiControl => GateMatrix {
            m00: [1.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [1.0, 0.0],
        },
        GateKind::Write0
        | GateKind::Write1
        | GateKind::BlochDisplay
        | GateKind::Measurement => GateMatrix {
            // BlochDisplay/Measurement are non-mutating viewers (Measurement
            // collapses on the CPU side); Write0/Write1 are mode-driven on the
            // GPU. Matrix is unused for these but filled with identity for safety.
            m00: [1.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [1.0, 0.0],
        },
        GateKind::X => GateMatrix {
            m00: [0.0, 0.0],
            m01: [1.0, 0.0],
            m10: [1.0, 0.0],
            m11: [0.0, 0.0],
        },
        GateKind::Y => GateMatrix {
            m00: [0.0, 0.0],
            m01: [0.0, -1.0],
            m10: [0.0, 1.0],
            m11: [0.0, 0.0],
        },
        GateKind::Z => GateMatrix {
            m00: [1.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [-1.0, 0.0],
        },
        GateKind::SqrtX => GateMatrix {
            m00: [0.5, 0.5],
            m01: [0.5, -0.5],
            m10: [0.5, -0.5],
            m11: [0.5, 0.5],
        },
        GateKind::S => GateMatrix {
            m00: [1.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [0.0, 1.0],
        },
        GateKind::SDagger => GateMatrix {
            m00: [1.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [0.0, -1.0],
        },
        GateKind::T => GateMatrix {
            m00: [1.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [inv_sqrt2, inv_sqrt2],
        },
        GateKind::TDagger => GateMatrix {
            m00: [1.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [inv_sqrt2, -inv_sqrt2],
        },
        GateKind::Phase => GateMatrix {
            m00: [1.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: exp_i(default_angle),
        },
        GateKind::Rx => GateMatrix {
            m00: [cos_half, 0.0],
            m01: [0.0, -sin_half],
            m10: [0.0, -sin_half],
            m11: [cos_half, 0.0],
        },
        GateKind::Ry => GateMatrix {
            m00: [cos_half, 0.0],
            m01: [-sin_half, 0.0],
            m10: [sin_half, 0.0],
            m11: [cos_half, 0.0],
        },
        GateKind::Rz => GateMatrix {
            m00: [cos_half, -sin_half],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [cos_half, sin_half],
        },
        GateKind::Swap => GateMatrix {
            m00: [1.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [1.0, 0.0],
        },
    }
}

// Per-gate matrices feed the CPU simulator in `bloch.rs`. The legacy
// `GateParams` GPU compute pipeline has been retired in favour of CPU-side
// simulation + a single state-vector upload (see `gpu.rs`).
