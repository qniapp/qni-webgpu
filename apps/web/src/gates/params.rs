use super::ColumnControls;
use super::GateKind;
use crate::gpu::{Measurement, SlotIndex};
use crate::qubit_bit::QubitBit;

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
        GateKind::Control
        | GateKind::AntiControl
        | GateKind::ZParityControl
        | GateKind::XParityControl => GateMatrix {
            m00: [1.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [1.0, 0.0],
        },
        GateKind::Write0
        | GateKind::Write1
        | GateKind::ZDetectControlReset
        | GateKind::XDetector
        | GateKind::YDetector
        | GateKind::Spacer => GateMatrix {
            // Write1 is mode-driven. Write0, detectors, and spacers leave this
            // unused matrix as identity.
            m00: [1.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [1.0, 0.0],
        },
        GateKind::BlochDisplay
        | GateKind::ProbabilityDisplay
        | GateKind::SampleDisplay
        | GateKind::AmplitudeDisplay
        | GateKind::DensityMatrixDisplay
        | GateKind::Measurement
        | GateKind::QftGate
        | GateKind::QftDaggerGate
        | GateKind::Increment
        | GateKind::Decrement => panic!("{kind:?} has no gate matrix"),
        GateKind::PostSelectOff => GateMatrix {
            m00: [1.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [0.0, 0.0],
        },
        GateKind::PostSelectOn => GateMatrix {
            m00: [0.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [1.0, 0.0],
        },
        GateKind::PostSelectAntiX => GateMatrix {
            m00: [0.5, 0.0],
            m01: [0.5, 0.0],
            m10: [0.5, 0.0],
            m11: [0.5, 0.0],
        },
        GateKind::PostSelectX => GateMatrix {
            m00: [0.5, 0.0],
            m01: [-0.5, 0.0],
            m10: [-0.5, 0.0],
            m11: [0.5, 0.0],
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

pub(crate) const GATE_MODE_MATRIX: u32 = 0;
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
    /// Measurement aux slot whose sampled outcome must be 1 for the gate to
    /// apply (qni `X<name`), or `GATE_UNCONDITIONAL`.
    condition_slot: u32,
    _pad: [u32; 2],
}

pub(crate) const GATE_UNCONDITIONAL: u32 = u32::MAX;

impl GateParams {
    pub(crate) fn conditioned_on(mut self, slot: SlotIndex<Measurement>) -> Self {
        self.condition_slot = slot.as_u32();
        self
    }
}

#[cfg(test)]
impl GateParams {
    pub(crate) fn matrix(self) -> [[f32; 2]; 4] {
        [self.m00, self.m01, self.m10, self.m11]
    }

    pub(crate) fn bit(self) -> u32 {
        self.bit
    }

    pub(crate) fn control_mask(self) -> u32 {
        self.control_mask
    }

    pub(crate) fn control_value(self) -> u32 {
        self.control_value
    }

    pub(crate) fn condition_slot(self) -> u32 {
        self.condition_slot
    }

    pub(crate) fn mode(self) -> u32 {
        self.mode
    }
}

fn gate_mode(kind: GateKind) -> u32 {
    match kind {
        GateKind::Write0 => panic!("Write0 must be lowered into measurement and conditional X"),
        GateKind::Write1 => GATE_MODE_WRITE1,
        _ => GATE_MODE_MATRIX,
    }
}

pub(crate) fn gate_params(kind: GateKind, bit: QubitBit, state_count: u32) -> GateParams {
    let matrix = gate_matrix(kind);
    GateParams {
        m00: matrix.m00,
        m01: matrix.m01,
        m10: matrix.m10,
        m11: matrix.m11,
        bit: bit.as_u32(),
        state_count,
        control_mask: 0,
        control_value: 0,
        mode: gate_mode(kind),
        condition_slot: GATE_UNCONDITIONAL,
        _pad: [0; 2],
    }
}

pub(crate) fn gate_params_controlled(
    kind: GateKind,
    bit: QubitBit,
    controls: ColumnControls,
    state_count: u32,
) -> GateParams {
    let matrix = gate_matrix(kind);
    GateParams {
        m00: matrix.m00,
        m01: matrix.m01,
        m10: matrix.m10,
        m11: matrix.m11,
        bit: bit.as_u32(),
        state_count,
        control_mask: controls.mask(),
        control_value: controls.value(),
        mode: gate_mode(kind),
        condition_slot: GATE_UNCONDITIONAL,
        _pad: [0; 2],
    }
}

pub(crate) fn phase_params(
    phase: f32,
    bit: QubitBit,
    controls: ColumnControls,
    state_count: u32,
) -> GateParams {
    GateParams {
        m00: [1.0, 0.0],
        m01: [0.0, 0.0],
        m10: [0.0, 0.0],
        m11: [phase.cos(), phase.sin()],
        bit: bit.as_u32(),
        state_count,
        control_mask: controls.mask(),
        control_value: controls.value(),
        mode: gate_mode(GateKind::Phase),
        condition_slot: GATE_UNCONDITIONAL,
        _pad: [0; 2],
    }
}

pub(crate) fn rx_params(
    theta: f32,
    bit: QubitBit,
    controls: ColumnControls,
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
        bit: bit.as_u32(),
        state_count,
        control_mask: controls.mask(),
        control_value: controls.value(),
        mode: gate_mode(GateKind::Rx),
        condition_slot: GATE_UNCONDITIONAL,
        _pad: [0; 2],
    }
}

pub(crate) fn ry_params(
    theta: f32,
    bit: QubitBit,
    controls: ColumnControls,
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
        bit: bit.as_u32(),
        state_count,
        control_mask: controls.mask(),
        control_value: controls.value(),
        mode: gate_mode(GateKind::Ry),
        condition_slot: GATE_UNCONDITIONAL,
        _pad: [0; 2],
    }
}

pub(crate) fn rz_params(
    theta: f32,
    bit: QubitBit,
    controls: ColumnControls,
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
        bit: bit.as_u32(),
        state_count,
        control_mask: controls.mask(),
        control_value: controls.value(),
        mode: gate_mode(GateKind::Rz),
        condition_slot: GATE_UNCONDITIONAL,
        _pad: [0; 2],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[should_panic(expected = "BlochDisplay has no gate matrix")]
    fn bloch_display_has_no_gate_matrix() {
        let _ = gate_matrix(GateKind::BlochDisplay);
    }

    #[test]
    #[should_panic(expected = "ProbabilityDisplay has no gate matrix")]
    fn probability_display_has_no_gate_matrix() {
        let _ = gate_matrix(GateKind::ProbabilityDisplay);
    }

    #[test]
    #[should_panic(expected = "SampleDisplay has no gate matrix")]
    fn sample_display_has_no_gate_matrix() {
        let _ = gate_matrix(GateKind::SampleDisplay);
    }

    #[test]
    #[should_panic(expected = "AmplitudeDisplay has no gate matrix")]
    fn amplitude_display_has_no_gate_matrix() {
        let _ = gate_matrix(GateKind::AmplitudeDisplay);
    }

    #[test]
    #[should_panic(expected = "DensityMatrixDisplay has no gate matrix")]
    fn density_display_has_no_gate_matrix() {
        let _ = gate_matrix(GateKind::DensityMatrixDisplay);
    }

    #[test]
    #[should_panic(expected = "Measurement has no gate matrix")]
    fn measurement_has_no_gate_matrix() {
        let _ = gate_matrix(GateKind::Measurement);
    }

    #[test]
    #[should_panic(expected = "QftGate has no gate matrix")]
    fn qft_has_no_gate_matrix() {
        let _ = gate_matrix(GateKind::QftGate);
    }

    #[test]
    #[should_panic(expected = "QftDaggerGate has no gate matrix")]
    fn qft_dagger_has_no_gate_matrix() {
        let _ = gate_matrix(GateKind::QftDaggerGate);
    }

    #[test]
    #[should_panic(expected = "Increment has no gate matrix")]
    fn increment_has_no_gate_matrix() {
        let _ = gate_matrix(GateKind::Increment);
    }

    #[test]
    #[should_panic(expected = "Decrement has no gate matrix")]
    fn decrement_has_no_gate_matrix() {
        let _ = gate_matrix(GateKind::Decrement);
    }

    #[test]
    fn postselection_off_matrix() {
        let kind = GateKind::from_url_token("|0⟩⟨0|").unwrap();
        let matrix = gate_matrix(kind);
        assert_eq!(
            [matrix.m00, matrix.m01, matrix.m10, matrix.m11],
            [[1.0, 0.0], [0.0, 0.0], [0.0, 0.0], [0.0, 0.0]]
        );
    }
    #[test]
    fn postselection_on_matrix() {
        let kind = GateKind::from_url_token("|1⟩⟨1|").unwrap();
        let matrix = gate_matrix(kind);
        assert_eq!(
            [matrix.m00, matrix.m01, matrix.m10, matrix.m11],
            [[0.0, 0.0], [0.0, 0.0], [0.0, 0.0], [1.0, 0.0]]
        );
    }
    #[test]
    fn postselection_plus_matrix() {
        let kind = GateKind::from_url_token("|+⟩⟨+|").unwrap();
        let matrix = gate_matrix(kind);
        assert_eq!(
            [matrix.m00, matrix.m01, matrix.m10, matrix.m11],
            [[0.5, 0.0], [0.5, 0.0], [0.5, 0.0], [0.5, 0.0]]
        );
    }
    #[test]
    fn postselection_minus_matrix() {
        let kind = GateKind::from_url_token("|-⟩⟨-|").unwrap();
        let matrix = gate_matrix(kind);
        assert_eq!(
            [matrix.m00, matrix.m01, matrix.m10, matrix.m11],
            [[0.5, 0.0], [-0.5, 0.0], [-0.5, 0.0], [0.5, 0.0]]
        );
    }
}
