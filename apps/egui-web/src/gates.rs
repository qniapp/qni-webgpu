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
    /// Simulation is deferred; for now the gate is a placeholder that
    /// renders but does not affect the state vector.
    QftGate,
    /// Inverse QFT.
    QftDaggerGate,
}

impl GateKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            GateKind::H => "H",
            GateKind::Control => "C",
            GateKind::AntiControl => "◦",
            GateKind::BlochDisplay => "B",
            GateKind::Measurement => "M",
            GateKind::Spacer => "…",
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
            GateKind::QftGate => "QFT",
            GateKind::QftDaggerGate => "QFT†",
        }
    }

    /// Is this a multi-qubit gate whose vertical span is user-controlled
    /// via the hover-revealed resize handle? Right now QFT / QFT† are the
    /// only such gates.
    pub(crate) fn is_resizable_span(self) -> bool {
        matches!(self, GateKind::QftGate | GateKind::QftDaggerGate)
    }

    /// Tooltip metadata for the palette hover popup. Strings, paragraph
    /// breaks, and `transitions` mirror
    /// `apps/www/app/views/application/_palette_help_templates.html.erb`
    /// in the qni repo so a user familiar with one app reads identical
    /// copy in the other. Parametric gates (Phase / Rx / Ry / Rz) show
    /// the qni default θ = π/2 / φ = π/2 behaviour.
    pub(crate) fn info(self) -> GateInfo {
        // qni hard-codes its `QubitTransitionComponent` amplitudes per
        // gate. We mirror those literals directly as `Amp { re, im }`
        // structs so the slices live in static memory (a const-fn
        // helper here doesn't get auto-promoted to `'static`).
        //
        // Useful constants:
        //   |0⟩ = [(1, 0), (0, 0)],  |1⟩ = [(0, 0), (1, 0)]
        //   INV_SQRT2 ≈ 0.7071 = 1/√2 (qni writes it as the literal
        //   `0.7071`).
        const INV_SQRT2: f32 = std::f32::consts::FRAC_1_SQRT_2;
        const Z: Amp = Amp { re: 0.0, im: 0.0 };
        const ONE: Amp = Amp { re: 1.0, im: 0.0 };
        const NEG_ONE: Amp = Amp { re: -1.0, im: 0.0 };
        const PI: Amp = Amp { re: 0.0, im: 1.0 }; // i
        const NEG_I: Amp = Amp { re: 0.0, im: -1.0 };
        const ISQ: Amp = Amp {
            re: INV_SQRT2,
            im: 0.0,
        };
        const NEG_ISQ: Amp = Amp {
            re: -INV_SQRT2,
            im: 0.0,
        };
        const NEG_I_ISQ: Amp = Amp {
            re: 0.0,
            im: -INV_SQRT2,
        };
        const HALF_PLUS_HALF_I: Amp = Amp { re: 0.5, im: 0.5 };
        const HALF_MINUS_HALF_I: Amp = Amp { re: 0.5, im: -0.5 };
        const ISQ_PLUS_I_ISQ: Amp = Amp {
            re: INV_SQRT2,
            im: INV_SQRT2,
        };
        const ISQ_MINUS_I_ISQ: Amp = Amp {
            re: INV_SQRT2,
            im: -INV_SQRT2,
        };
        const KET0: [Amp; 2] = [ONE, Z];
        const KET1: [Amp; 2] = [Z, ONE];

        match self {
            GateKind::H => GateInfo {
                name: "Hadamard Gate",
                paragraphs: &["Creates simple superpositions."],
                transitions: &[
                    GateTransition {
                        from: KET0,
                        to: [ISQ, ISQ],
                    },
                    GateTransition {
                        from: KET1,
                        to: [ISQ, NEG_ISQ],
                    },
                ],
            },
            GateKind::X => GateInfo {
                name: "NOT Gate (Pauli X Gate)",
                paragraphs: &["Swaps the qubit pair of |0⟩ and |1⟩."],
                transitions: &[
                    GateTransition {
                        from: KET0,
                        to: KET1,
                    },
                    GateTransition {
                        from: KET1,
                        to: KET0,
                    },
                ],
            },
            GateKind::Y => GateInfo {
                name: "Pauli Y Gate",
                paragraphs: &["A combination of the X and Z gates."],
                transitions: &[
                    GateTransition {
                        from: KET0,
                        to: [Z, PI],
                    },
                    GateTransition {
                        from: KET1,
                        to: [NEG_I, Z],
                    },
                ],
            },
            GateKind::Z => GateInfo {
                name: "Pauli Z Gate",
                paragraphs: &["Negate phases when the qubit is ON."],
                transitions: &[
                    GateTransition {
                        from: KET0,
                        to: KET0,
                    },
                    GateTransition {
                        from: KET1,
                        to: [Z, NEG_ONE],
                    },
                ],
            },
            GateKind::SqrtX => GateInfo {
                name: "Square Root of NOT Gate",
                paragraphs: &["Rotates around the X-axis by π/2."],
                transitions: &[
                    GateTransition {
                        from: KET0,
                        to: [HALF_PLUS_HALF_I, HALF_MINUS_HALF_I],
                    },
                    GateTransition {
                        from: KET1,
                        to: [HALF_MINUS_HALF_I, HALF_PLUS_HALF_I],
                    },
                ],
            },
            GateKind::S => GateInfo {
                name: "S Gate",
                paragraphs: &[
                    "A shortcut for π/2 Phase gate.",
                    "Applies a phase of e^(iπ/2) to the |1⟩ state.",
                ],
                transitions: &[
                    GateTransition {
                        from: KET0,
                        to: KET0,
                    },
                    GateTransition {
                        from: KET1,
                        to: [Z, PI],
                    },
                ],
            },
            GateKind::SDagger => GateInfo {
                name: "S† Gate",
                paragraphs: &[
                    "A shortcut for -π/2 Phase gate.",
                    "Applies a phase of e^(-iπ/2) to the |1⟩ state.",
                ],
                transitions: &[
                    GateTransition {
                        from: KET0,
                        to: KET0,
                    },
                    GateTransition {
                        from: KET1,
                        to: [Z, NEG_I],
                    },
                ],
            },
            GateKind::T => GateInfo {
                name: "T Gate",
                paragraphs: &[
                    "A shortcut for π/4 Phase gate.",
                    "Applies a phase of e^(iπ/4) to the |1⟩ state.",
                ],
                transitions: &[
                    GateTransition {
                        from: KET0,
                        to: KET0,
                    },
                    GateTransition {
                        from: KET1,
                        to: [Z, ISQ_PLUS_I_ISQ],
                    },
                ],
            },
            GateKind::TDagger => GateInfo {
                name: "T† Gate",
                paragraphs: &[
                    "A shortcut for -π/4 Phase gate.",
                    "Applies a phase of e^(-iπ/4) to the |1⟩ state.",
                ],
                transitions: &[
                    GateTransition {
                        from: KET0,
                        to: KET0,
                    },
                    GateTransition {
                        from: KET1,
                        to: [Z, ISQ_MINUS_I_ISQ],
                    },
                ],
            },
            GateKind::Phase => GateInfo {
                name: "Phase Gate",
                paragraphs: &[
                    "Applies a phase of e^(iφ) to the |1⟩ state.",
                    "The default φ = π/2.",
                ],
                transitions: &[
                    GateTransition {
                        from: KET0,
                        to: KET0,
                    },
                    GateTransition {
                        from: KET1,
                        to: [Z, PI],
                    },
                ],
            },
            GateKind::Rx => GateInfo {
                name: "Rx Gate",
                paragraphs: &[
                    "Rotates around the X-axis by the angle θ.",
                    "The default θ = π/2.",
                ],
                transitions: &[
                    GateTransition {
                        from: KET0,
                        to: [ISQ, NEG_I_ISQ],
                    },
                    GateTransition {
                        from: KET1,
                        to: [NEG_I_ISQ, ISQ],
                    },
                ],
            },
            GateKind::Ry => GateInfo {
                name: "Ry Gate",
                paragraphs: &[
                    "Rotates around the Y-axis by the angle θ.",
                    "The default θ = π/2.",
                ],
                transitions: &[
                    GateTransition {
                        from: KET0,
                        to: [ISQ, ISQ],
                    },
                    GateTransition {
                        from: KET1,
                        to: [NEG_ISQ, ISQ],
                    },
                ],
            },
            GateKind::Rz => GateInfo {
                name: "Rz Gate",
                paragraphs: &[
                    "Rotates around the Z-axis by the angle θ.",
                    "The default θ = π/2.",
                ],
                transitions: &[
                    GateTransition {
                        from: KET0,
                        to: [ISQ_MINUS_I_ISQ, Z],
                    },
                    GateTransition {
                        from: KET1,
                        to: [Z, ISQ_PLUS_I_ISQ],
                    },
                ],
            },
            GateKind::QftGate => GateInfo {
                name: "Quantum Fourier Transform Gate",
                paragraphs: &["Transforms to/from phase frequency space."],
                transitions: &[],
            },
            GateKind::QftDaggerGate => GateInfo {
                name: "Inverse Quantum Fourier Transform Gate",
                paragraphs: &["Transforms to/from phase frequency space."],
                transitions: &[],
            },
            GateKind::Swap => GateInfo {
                name: "Swap Gate",
                paragraphs: &[
                    "Swap the values of two qubits.",
                    "(Place two in the same step.)",
                ],
                transitions: &[],
            },
            GateKind::Control => GateInfo {
                name: "Control Gate",
                paragraphs: &[
                    "Conditions on a qubit being ON.",
                    "Gates in the same step only apply to states meeting the condition.",
                ],
                transitions: &[],
            },
            GateKind::AntiControl => GateInfo {
                name: "Anti Control Gate",
                paragraphs: &[
                    "Conditions on a qubit being OFF.",
                    "Gates in the same step only apply to states meeting the condition.",
                ],
                transitions: &[],
            },
            GateKind::BlochDisplay => GateInfo {
                name: "Bloch Sphere Display",
                paragraphs: &["Shows a wire's local state as a point on the Bloch sphere."],
                transitions: &[],
            },
            GateKind::Write0 => GateInfo {
                name: "|0⟩ Operation",
                paragraphs: &["(Re)sets a qubit to state |0⟩."],
                transitions: &[
                    GateTransition {
                        from: KET0,
                        to: KET0,
                    },
                    GateTransition {
                        from: KET1,
                        to: KET0,
                    },
                ],
            },
            GateKind::Write1 => GateInfo {
                name: "|1⟩ Operation",
                paragraphs: &["(Re)sets a qubit to state |1⟩."],
                transitions: &[
                    GateTransition {
                        from: KET0,
                        to: KET1,
                    },
                    GateTransition {
                        from: KET1,
                        to: KET1,
                    },
                ],
            },
            GateKind::Measurement => GateInfo {
                name: "Measurement Gate",
                paragraphs: &["Measures whether a qubit is ON or OFF."],
                transitions: &[],
            },
            GateKind::Spacer => GateInfo {
                name: "Spacer Gate",
                paragraphs: &["A gate with no effect."],
                transitions: &[],
            },
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct GateInfo {
    pub(crate) name: &'static str,
    /// One or more paragraphs of description text, each rendered on its
    /// own line. Mirrors qni's `<p>…</p>` blocks inside `<header>`.
    pub(crate) paragraphs: &'static [&'static str],
    /// One row per input basis state shown in the mini transformation
    /// diagram. Empty for gates that don't have a `QubitTransitionComponent`
    /// example in qni (multi-qubit gates, displays, controls, etc.).
    pub(crate) transitions: &'static [GateTransition],
}

/// Complex amplitude in the standard re + im·i form. Same shape as
/// `qni`'s ruby tuples (`[re, im]` / `'i'` / `'0.7071 + 0.7071i'`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Amp {
    pub(crate) re: f32,
    pub(crate) im: f32,
}

impl Amp {
    /// Probability (|amp|²) — drives the filled-disk radius of the
    /// amplitude circle in the tooltip diagram.
    pub(crate) fn probability(self) -> f32 {
        self.re * self.re + self.im * self.im
    }

    /// Phase in radians, where 0 means the needle points up
    /// (12 o'clock) — matching the state-panel convention.
    pub(crate) fn phase(self) -> f32 {
        self.im.atan2(self.re)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct GateTransition {
    /// Pre-gate qubit state as 2 amplitudes for the |0⟩ / |1⟩ basis.
    pub(crate) from: [Amp; 2],
    /// Post-gate qubit state. qni hard-codes these per gate (using the
    /// default angle for parametric gates).
    pub(crate) to: [Amp; 2],
}

#[derive(Clone, Copy, Debug)]
struct GateMatrix {
    m00: [f32; 2],
    m01: [f32; 2],
    m10: [f32; 2],
    m11: [f32; 2],
}

fn gate_matrix(kind: GateKind) -> GateMatrix {
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
        | GateKind::Measurement
        | GateKind::Spacer
        | GateKind::QftGate
        | GateKind::QftDaggerGate => GateMatrix {
            // BlochDisplay/Measurement are non-mutating viewers (Measurement
            // collapses on the CPU side); Write0/Write1 are mode-driven on the
            // GPU. QFT / QFT† simulation is deferred — for now the gate is a
            // UI-only placeholder that renders but does not affect the state.
            // Matrix is unused for these but filled with identity for safety.
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

// `GateParams` is the on-wire layout of a single gate operation handed to the
// WGSL compute shader (`STATE_COMPUTE_SHADER` in `gpu.rs`). Each placed gate is
// linearised into one of these so the GPU can apply it via per-pair matrix
// multiply or, for Write0/Write1, a per-pair conditional swap.
pub(crate) const GATE_MODE_MATRIX: u32 = 0;
pub(crate) const GATE_MODE_WRITE0: u32 = 1;
pub(crate) const GATE_MODE_WRITE1: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GateParams {
    m00: [f32; 2],
    m01: [f32; 2],
    m10: [f32; 2],
    m11: [f32; 2],
    bit: u32,
    state_count: u32,
    control_mask: u32,
    control_value: u32,
    mode: u32,
    _pad: [u32; 3],
}

fn gate_mode(kind: GateKind) -> u32 {
    match kind {
        GateKind::Write0 => GATE_MODE_WRITE0,
        GateKind::Write1 => GATE_MODE_WRITE1,
        _ => GATE_MODE_MATRIX,
    }
}

pub(crate) fn gate_params(kind: GateKind, bit: u32, state_count: u32) -> GateParams {
    let matrix = gate_matrix(kind);
    GateParams {
        m00: matrix.m00,
        m01: matrix.m01,
        m10: matrix.m10,
        m11: matrix.m11,
        bit,
        state_count,
        control_mask: 0,
        control_value: 0,
        mode: gate_mode(kind),
        _pad: [0; 3],
    }
}

pub(crate) fn gate_params_controlled(
    kind: GateKind,
    bit: u32,
    control_mask: u32,
    control_value: u32,
    state_count: u32,
) -> GateParams {
    let matrix = gate_matrix(kind);
    GateParams {
        m00: matrix.m00,
        m01: matrix.m01,
        m10: matrix.m10,
        m11: matrix.m11,
        bit,
        state_count,
        control_mask,
        control_value,
        mode: gate_mode(kind),
        _pad: [0; 3],
    }
}

/// Parametric `P(φ)` phase gate: `diag(1, e^{iφ})`. Mirrors qni's
/// `gate-matrices.ts::PHASE` (`/packages/simulator/src/gate-matrices.ts:116`).
/// `control_mask` / `control_value` follow the standard control-mask
/// convention — pass `0`/`0` for an uncontrolled phase, or a bit mask
/// for a controlled-phase as part of a CZ-style column.
pub(crate) fn phase_params(
    phase: f32,
    bit: u32,
    control_mask: u32,
    control_value: u32,
    state_count: u32,
) -> GateParams {
    GateParams {
        m00: [1.0, 0.0],
        m01: [0.0, 0.0],
        m10: [0.0, 0.0],
        m11: [phase.cos(), phase.sin()],
        bit,
        state_count,
        control_mask,
        control_value,
        mode: gate_mode(GateKind::Phase),
        _pad: [0; 3],
    }
}

/// Parametric `Rx(θ)` rotation gate: `[[cos(θ/2), -i sin(θ/2)],
/// [-i sin(θ/2), cos(θ/2)]]`. Mirrors qni's
/// `gate-matrices.ts::RX` family. `control_mask` / `control_value`
/// follow the standard convention (pass `0`/`0` for uncontrolled).
pub(crate) fn rx_params(
    theta: f32,
    bit: u32,
    control_mask: u32,
    control_value: u32,
    state_count: u32,
) -> GateParams {
    let half = theta * 0.5;
    let c = half.cos();
    let s = half.sin();
    GateParams {
        m00: [c, 0.0],
        m01: [0.0, -s],
        m10: [0.0, -s],
        m11: [c, 0.0],
        bit,
        state_count,
        control_mask,
        control_value,
        mode: gate_mode(GateKind::Rx),
        _pad: [0; 3],
    }
}

/// Parametric `Ry(θ)` rotation gate: `[[cos(θ/2), -sin(θ/2)],
/// [sin(θ/2), cos(θ/2)]]`.
pub(crate) fn ry_params(
    theta: f32,
    bit: u32,
    control_mask: u32,
    control_value: u32,
    state_count: u32,
) -> GateParams {
    let half = theta * 0.5;
    let c = half.cos();
    let s = half.sin();
    GateParams {
        m00: [c, 0.0],
        m01: [-s, 0.0],
        m10: [s, 0.0],
        m11: [c, 0.0],
        bit,
        state_count,
        control_mask,
        control_value,
        mode: gate_mode(GateKind::Ry),
        _pad: [0; 3],
    }
}

/// Parametric `Rz(θ)` rotation gate: `diag(e^{-iθ/2}, e^{iθ/2})`.
pub(crate) fn rz_params(
    theta: f32,
    bit: u32,
    control_mask: u32,
    control_value: u32,
    state_count: u32,
) -> GateParams {
    let half = theta * 0.5;
    let c = half.cos();
    let s = half.sin();
    GateParams {
        m00: [c, -s],
        m01: [0.0, 0.0],
        m10: [0.0, 0.0],
        m11: [c, s],
        bit,
        state_count,
        control_mask,
        control_value,
        mode: gate_mode(GateKind::Rz),
        _pad: [0; 3],
    }
}

/// Parse qni's URL angle string into radians. Mirrors qni's
/// `angle-parser.ts::radian` + `piCoefficient`
/// (`/packages/common/src/angle-parser.ts:3-62`):
///
/// * `"π"`            → π
/// * `"-π"`           → -π
/// * `"π/2"` / `"π_2"`→ π/2  (URL stores `_` in place of `/`)
/// * `"-π/128"`       → -π/128
/// * `"2π/3"`         → 2π/3
/// * `"0"`            → 0
///
/// Returns `None` for unparseable input. The string must contain `π`
/// unless it is exactly `"0"` (matches qni's `isValidAngle`).
pub(crate) fn parse_angle_radians(s: &str) -> Option<f32> {
    let trimmed = s.trim();
    if trimmed == "0" {
        return Some(0.0);
    }
    // Replace first `_` with `/` per qni's `replace('_','/')` (non-global).
    let with_slash = if let Some(idx) = trimmed.find('_') {
        let mut out = String::with_capacity(trimmed.len());
        out.push_str(&trimmed[..idx]);
        out.push('/');
        out.push_str(&trimmed[idx + 1..]);
        out
    } else {
        trimmed.to_string()
    };
    if !with_slash.contains('π') {
        return None;
    }
    // piCoefficient: strip `π`; bare `π` becomes `"1"`, `Nπ` becomes `"N"`.
    let coefficient = pi_coefficient(&with_slash);
    let value = parse_fraction(&coefficient)?;
    Some(value * std::f32::consts::PI)
}

/// Strip the `π` literal from an angle string, leaving its numeric
/// coefficient. `"π"` → `"1"`, `"-π"` → `"-1"`, `"3π"` → `"3"`,
/// `"π/2"` → `"1/2"`, `"-π/128"` → `"-1/128"`, `"2π/3"` → `"2/3"`.
fn pi_coefficient(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c == 'π' {
            // If the immediately preceding output is a digit, just drop
            // the `π` (we already captured the coefficient). Otherwise
            // (start of string, after a sign, after `/`), the `π`
            // stands alone — emit `1` in its place.
            let last = out.chars().last();
            if !matches!(last, Some(ch) if ch.is_ascii_digit()) {
                out.push('1');
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Parse `"a/b"` (or bare `"a"`) as f32. Both sides may carry a sign.
fn parse_fraction(s: &str) -> Option<f32> {
    if let Some((num, denom)) = s.split_once('/') {
        let n: f32 = num.trim().parse().ok()?;
        let d: f32 = denom.trim().parse().ok()?;
        if d == 0.0 {
            return None;
        }
        Some(n / d)
    } else {
        s.trim().parse().ok()
    }
}

/// Single-control phase gate with an arbitrary phase angle. Used by the
/// QFT / QFT† decomposition (the textbook `R_k = diag(1, e^{iπ/2^j})`
/// rotations between every pair of qubits in the QFT span). Unlike the
/// fixed-phase `GateKind::Phase` (π/2), the angle here is a runtime
/// parameter; everything else (matrix shape, mode) is identical.
pub(crate) fn controlled_phase_params(
    target_bit: u32,
    control_bit: u32,
    phase: f32,
    state_count: u32,
) -> GateParams {
    GateParams {
        m00: [1.0, 0.0],
        m01: [0.0, 0.0],
        m10: [0.0, 0.0],
        m11: [phase.cos(), phase.sin()],
        bit: target_bit,
        state_count,
        control_mask: 1u32 << control_bit,
        control_value: 1u32 << control_bit,
        mode: GATE_MODE_MATRIX,
        _pad: [0; 3],
    }
}
