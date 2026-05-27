//! Gate matrix packing and qni-compatible angle parsing.

use super::GateKind;

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
        | GateKind::ProbabilityDisplay
        | GateKind::AmplitudeDisplay
        | GateKind::DensityMatrixDisplay
        | GateKind::Spacer
        | GateKind::QftGate
        | GateKind::QftDaggerGate => GateMatrix {
            // BlochDisplay / Measurement / Probability / Amplitude / Density / QFT are handled by dedicated GPU
            // orchestration paths instead of this 2x2 matrix helper. Write0 /
            // Write1 are mode-driven on the GPU. Matrix is unused for these
            // variants but filled with identity for safety.
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
    let radians = value * std::f32::consts::PI;
    radians.is_finite().then_some(radians)
}

pub(crate) fn normalize_angle_input(input: &str) -> Option<String> {
    let compact: String = input
        .trim()
        .replace('−', "-")
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect();
    if compact.is_empty() {
        return None;
    }
    let coefficient = if compact.contains('π') {
        match pi_input_coefficient(&compact) {
            Some(coefficient) => coefficient,
            None if valid_decimal_pi_input(&compact) => return Some(compact),
            None => return None,
        }
    } else {
        compact
    };
    let (numerator, denominator) = parse_integer_fraction(&coefficient)?;
    let normalized = format_pi_coefficient(numerator, denominator);
    parse_angle_radians(&normalized)?;
    Some(normalized)
}

fn pi_input_coefficient(s: &str) -> Option<String> {
    if s.matches('π').count() != 1 {
        return None;
    }
    let pi_index = s.find('π')?;
    let before = &s[..pi_index];
    let after = &s[pi_index + 'π'.len_utf8()..];
    if let Some((numerator, denominator)) = before.split_once('/') {
        if !after.is_empty() || !signed_integer(numerator) || !signed_integer(denominator) {
            return None;
        }
        return Some(format!("{numerator}/{denominator}"));
    }
    let numerator = match before {
        "" | "+" => "1".to_owned(),
        "-" => "-1".to_owned(),
        value if signed_integer(value) => value.to_owned(),
        _ => return None,
    };
    if after.is_empty() {
        return Some(numerator);
    }
    let denominator = after.strip_prefix('/')?;
    if !signed_integer(denominator) {
        return None;
    }
    Some(format!("{numerator}/{denominator}"))
}

fn signed_integer(s: &str) -> bool {
    let digits = s
        .strip_prefix(['+', '-'])
        .filter(|rest| !rest.is_empty())
        .unwrap_or(s);
    !digits.is_empty() && digits.chars().all(|ch| ch.is_ascii_digit())
}

fn valid_decimal_pi_input(s: &str) -> bool {
    if s.matches('π').count() != 1 {
        return false;
    }
    let Some(before) = s.strip_suffix('π') else {
        return false;
    };
    before.contains('.')
        && before.parse::<f32>().is_ok_and(f32::is_finite)
        && parse_angle_radians(s).is_some()
}

fn parse_integer_fraction(s: &str) -> Option<(i128, i128)> {
    let (mut numerator, mut denominator): (i128, i128) =
        if let Some((num, denom)) = s.split_once('/') {
            (num.parse().ok()?, denom.parse().ok()?)
        } else {
            (s.parse().ok()?, 1)
        };
    if denominator == 0 || numerator == i128::MIN || denominator == i128::MIN {
        return None;
    }
    if denominator < 0 {
        numerator = numerator.checked_neg()?;
        denominator = denominator.checked_neg()?;
    }
    let divisor = gcd_i128(numerator, denominator);
    Some((numerator / divisor, denominator / divisor))
}

fn gcd_i128(a: i128, b: i128) -> i128 {
    let mut a = a.abs();
    let mut b = b.abs();
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a.max(1)
}

fn format_pi_coefficient(numerator: i128, denominator: i128) -> String {
    if numerator == 0 {
        return "0".to_owned();
    }
    let sign = if numerator < 0 { "-" } else { "" };
    let abs_numerator = numerator.abs();
    if denominator == 1 {
        if abs_numerator == 1 {
            format!("{sign}π")
        } else {
            format!("{sign}{abs_numerator}π")
        }
    } else if abs_numerator == 1 {
        format!("{sign}π/{denominator}")
    } else {
        format!("{sign}{abs_numerator}π/{denominator}")
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_angle_fraction_adds_pi() {
        assert_eq!(normalize_angle_input("1/2"), Some("π/2".to_owned()));
    }

    #[test]
    fn normalize_angle_reduces_fraction() {
        assert_eq!(normalize_angle_input("2/4"), Some("π/2".to_owned()));
    }

    #[test]
    fn normalize_angle_rejects_zero_denominator() {
        assert_eq!(normalize_angle_input("1/0"), None);
    }

    #[test]
    fn normalize_angle_integer_one_becomes_pi() {
        assert_eq!(normalize_angle_input("1"), Some("π".to_owned()));
    }

    #[test]
    fn normalize_angle_zero_stays_zero() {
        assert_eq!(normalize_angle_input("0"), Some("0".to_owned()));
    }

    #[test]
    fn normalize_angle_negative_fraction_keeps_sign() {
        assert_eq!(normalize_angle_input("-1/2"), Some("-π/2".to_owned()));
    }

    #[test]
    fn normalize_angle_double_negative_reduces_positive() {
        assert_eq!(normalize_angle_input("-2/-4"), Some("π/2".to_owned()));
    }

    #[test]
    fn normalize_angle_rejects_word() {
        assert_eq!(normalize_angle_input("abc"), None);
    }

    #[test]
    fn normalize_angle_rejects_double_slash() {
        assert_eq!(normalize_angle_input("1//2"), None);
    }

    #[test]
    fn normalize_angle_rejects_repeated_pi() {
        assert_eq!(normalize_angle_input("ππ"), None);
    }

    #[test]
    fn normalize_angle_rejects_pi_denominator() {
        assert_eq!(normalize_angle_input("π/π"), None);
    }

    #[test]
    fn normalize_angle_rejects_trailing_pi_coefficient() {
        assert_eq!(normalize_angle_input("1π2"), None);
    }

    #[test]
    fn normalize_angle_accepts_suffix_pi_fraction() {
        assert_eq!(normalize_angle_input("1/3π"), Some("π/3".to_owned()));
    }

    #[test]
    fn normalize_angle_rejects_suffix_and_postfix_denominator() {
        assert_eq!(normalize_angle_input("1/3π/2"), None);
    }

    #[test]
    fn normalize_angle_rejects_i128_min() {
        assert_eq!(
            normalize_angle_input("-170141183460469231731687303715884105728"),
            None
        );
    }

    #[test]
    fn normalize_angle_rejects_non_finite_radians() {
        assert_eq!(
            normalize_angle_input("170141183460469231731687303715884105727"),
            None
        );
    }

    #[test]
    fn normalize_angle_accepts_decimal_pi() {
        assert_eq!(normalize_angle_input("0.5π"), Some("0.5π".to_owned()));
    }
}
