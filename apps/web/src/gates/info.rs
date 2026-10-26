use super::GateKind;

#[derive(Clone, Copy, Debug)]
pub(crate) struct GateInfo {
    pub(crate) name: &'static str,
    pub(crate) paragraphs: &'static [&'static str],
    pub(crate) transitions: &'static [GateTransition],
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Amp {
    pub(crate) re: f32,
    pub(crate) im: f32,
}

impl Amp {
    pub(crate) fn probability(self) -> f32 {
        self.re * self.re + self.im * self.im
    }

    pub(crate) fn phase(self) -> f32 {
        self.im.atan2(self.re)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct GateTransition {
    pub(crate) from: [Amp; 2],
    pub(crate) to: [Amp; 2],
}

impl GateKind {
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
            GateKind::Increment => GateInfo {
                name: "Increment Gate",
                paragraphs: &[
                    "Adds one modulo 2^n to the covered register.",
                    "The top wire is the least-significant bit. Resize to cover 1–16 wires.",
                ],
                transitions: &[],
            },
            GateKind::Decrement => GateInfo {
                name: "Decrement Gate",
                paragraphs: &[
                    "Subtracts one modulo 2^n from the covered register.",
                    "The top wire is the least-significant bit. Resize to cover 1–16 wires.",
                ],
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
            GateKind::ZParityControl | GateKind::XParityControl => GateInfo {
                name: if self == GateKind::ZParityControl { "Z Parity Control" } else { "X Parity Control" },
                paragraphs: &[
                    "Applies the column when an odd number of parity controls are satisfied.",
                    "Zpar uses the Z basis; Xpar uses the X basis. Ordinary controls are combined by AND.",
                    "Available in local WebGPU mode only; the external Qiskit API does not support parity controls.",
                ],
                transitions: &[],
            },
            GateKind::SampleDisplay => GateInfo {
                name: "Sample Display",
                paragraphs: &[
                    "Shows one sampled bit string without changing or collapsing the state.",
                    "Use controls to see conditional samples. Resize to cover one to eight wires.",
                    "Available in local WebGPU mode only; the external Qiskit API does not support sampling displays.",
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
            GateKind::ZDetectControlReset => GateInfo {
                name: "Z Detect-Control-Reset",
                paragraphs: &[
                    "Samples Z at the start of the column.",
                    "Controls operations in this column when the result is 1, then resets to |0⟩ at the end.",
                    "Measurement gates ignore controls. Display blocks show conditional results.",
                ],
                transitions: &[],
            },
            GateKind::PostSelectOff => GateInfo {
                name: "Postselect |0⟩",
                paragraphs: &["Keeps only the |0⟩ outcome and renormalizes the surviving state."],
                transitions: &[],
            },
            GateKind::PostSelectOn => GateInfo {
                name: "Postselect |1⟩",
                paragraphs: &["Keeps only the |1⟩ outcome and renormalizes the surviving state."],
                transitions: &[],
            },
            GateKind::PostSelectAntiX => GateInfo {
                name: "Postselect |+⟩",
                paragraphs: &["Keeps only the |+⟩ outcome and renormalizes the surviving state."],
                transitions: &[],
            },
            GateKind::PostSelectX => GateInfo {
                name: "Postselect |−⟩",
                paragraphs: &["Keeps only the |−⟩ outcome and renormalizes the surviving state."],
                transitions: &[],
            },
            GateKind::XDetector => GateInfo {
                name: "X Detector",
                paragraphs: &[
                    "Samples the negative X eigenstate, or its joint subspace with column controls.",
                    "0 = no click; 1 = click. Without controls these mean +1 and -1 eigenvalues.",
                    "Without controls, equivalent to H → Measurement → H.",
                ],
                transitions: &[],
            },
            GateKind::YDetector => GateInfo {
                name: "Y Detector",
                paragraphs: &[
                    "Samples the negative Y eigenstate, or its joint subspace with column controls.",
                    "0 = no click; 1 = click. Without controls these mean +1 and -1 eigenvalues.",
                    "Without controls, equivalent to S† → H → Measurement → H → S.",
                ],
                transitions: &[],
            },
            GateKind::Measurement => GateInfo {
                name: "Measurement Gate",
                paragraphs: &["Measures whether a qubit is ON or OFF."],
                transitions: &[],
            },
            GateKind::ProbabilityDisplay => GateInfo {
                name: "Probability Display",
                paragraphs: &[
                    "Shows probabilities of outcomes if a measurement was performed.",
                    "Use controls to see conditional probabilities.",
                ],
                transitions: &[],
            },
            GateKind::AmplitudeDisplay => GateInfo {
                name: "Amplitude Display",
                paragraphs: &[
                    "Shows complex amplitudes for a span of qubits.",
                    "Use controls to see conditional amplitudes.",
                ],
                transitions: &[],
            },
            GateKind::DensityMatrixDisplay => GateInfo {
                name: "Density Matrix Display",
                paragraphs: &[
                    "Shows the reduced density matrix for a span of qubits.",
                    "Use controls to see conditional local mixed states.",
                ],
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
