use std::num::NonZeroU32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ParametricAngle {
    numerator: i32,
    denominator: NonZeroU32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParametricAngleError {
    Invalid,
}

impl ParametricAngle {
    pub(crate) const DEFAULT_LABEL: &'static str = "π/2";

    pub(crate) fn parse_qni(input: &str) -> Result<Self, ParametricAngleError> {
        parse_normalized_qni(trim_ascii(input))
    }

    pub(crate) fn parse_user_input(input: &str) -> Result<Self, ParametricAngleError> {
        let normalized = input
            .replace('−', "-")
            .replace("PI", "π")
            .replace("pi", "π")
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect::<String>();
        if normalized.contains('π') {
            Self::parse_qni(&normalized)
        } else {
            parse_pi_free_coefficient(&normalized)
        }
    }

    pub(crate) fn from_pi_fraction(numerator: i32, denominator: NonZeroU32) -> Self {
        normalize_fraction(numerator as i64, denominator.get() as u64)
            .expect("i32/u32 fraction should normalize into ParametricAngle")
    }

    pub(crate) fn label(&self) -> String {
        let numerator = self.numerator;
        if numerator == 0 {
            return "0".to_owned();
        }
        let sign = if numerator < 0 { "-" } else { "" };
        let coefficient = numerator.unsigned_abs();
        let denominator = self.denominator.get();
        match (coefficient, denominator) {
            (1, 1) => format!("{sign}π"),
            (_, 1) => format!("{sign}{coefficient}π"),
            (1, _) => format!("{sign}π/{denominator}"),
            _ => format!("{sign}{coefficient}π/{denominator}"),
        }
    }

    pub(crate) fn url_label(&self) -> String {
        self.label().replacen('/', "_", 1)
    }

    pub(crate) fn radians_f32(&self) -> f32 {
        (self.numerator as f32) * std::f32::consts::PI / (self.denominator.get() as f32)
    }

    #[allow(dead_code)]
    pub(crate) fn numerator(&self) -> i32 {
        self.numerator
    }

    #[allow(dead_code)]
    pub(crate) fn denominator(&self) -> u32 {
        self.denominator.get()
    }
}

impl Default for ParametricAngle {
    fn default() -> Self {
        debug_assert_eq!(Self::DEFAULT_LABEL, "π/2");
        Self::from_pi_fraction(1, NonZeroU32::new(2).expect("2 is non-zero"))
    }
}

fn trim_ascii(input: &str) -> &str {
    input.trim_matches(|ch: char| ch.is_ascii_whitespace())
}

fn parse_normalized_qni(input: &str) -> Result<ParametricAngle, ParametricAngleError> {
    if input.is_empty() {
        return Err(ParametricAngleError::Invalid);
    }
    let input = replace_first_url_separator(input);
    if input == "0" {
        return normalize_fraction(0, 1);
    }
    if input.chars().filter(|ch| *ch == 'π').count() != 1 {
        return Err(ParametricAngleError::Invalid);
    }
    let (before_pi, after_pi) = input.split_once('π').ok_or(ParametricAngleError::Invalid)?;
    let (numerator, denominator) = parse_pi_coefficient(before_pi, after_pi)?;
    normalize_fraction(numerator, denominator)
}

fn replace_first_url_separator(input: &str) -> String {
    if let Some(index) = input.find('_') {
        let mut output = String::with_capacity(input.len());
        output.push_str(&input[..index]);
        output.push('/');
        output.push_str(&input[index + 1..]);
        output
    } else {
        input.to_owned()
    }
}

fn parse_pi_coefficient(
    before_pi: &str,
    after_pi: &str,
) -> Result<(i64, u64), ParametricAngleError> {
    if before_pi.contains('/') {
        if !after_pi.is_empty() {
            return Err(ParametricAngleError::Invalid);
        }
        return parse_prefix_fraction(before_pi);
    }

    let numerator = match before_pi {
        "" | "+" => 1,
        "-" => -1,
        value => parse_signed_i64(value)?,
    };
    let denominator = if after_pi.is_empty() {
        1
    } else if let Some(raw_denominator) = after_pi.strip_prefix('/') {
        if raw_denominator.is_empty() {
            return Err(ParametricAngleError::Invalid);
        }
        parse_positive_u32(raw_denominator)? as u64
    } else {
        return Err(ParametricAngleError::Invalid);
    };
    Ok((numerator, denominator))
}

fn parse_prefix_fraction(input: &str) -> Result<(i64, u64), ParametricAngleError> {
    let (raw_numerator, raw_denominator) =
        input.split_once('/').ok_or(ParametricAngleError::Invalid)?;
    if raw_denominator.contains('/') {
        return Err(ParametricAngleError::Invalid);
    }
    Ok((
        parse_signed_i64(raw_numerator)?,
        parse_positive_u32(raw_denominator)? as u64,
    ))
}

fn parse_pi_free_coefficient(input: &str) -> Result<ParametricAngle, ParametricAngleError> {
    if input.is_empty() {
        return Err(ParametricAngleError::Invalid);
    }
    let (numerator, denominator) =
        if let Some((raw_numerator, raw_denominator)) = input.split_once('/') {
            if raw_denominator.contains('/') {
                return Err(ParametricAngleError::Invalid);
            }
            (
                parse_signed_i64(raw_numerator)?,
                parse_positive_u32(raw_denominator)? as u64,
            )
        } else {
            (parse_signed_i64(input)?, 1)
        };
    normalize_fraction(numerator, denominator)
}

fn parse_signed_i64(input: &str) -> Result<i64, ParametricAngleError> {
    let digits = input.strip_prefix(['+', '-']).unwrap_or(input);
    if digits.is_empty() || !digits.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(ParametricAngleError::Invalid);
    }
    input
        .parse::<i64>()
        .map_err(|_| ParametricAngleError::Invalid)
}

fn parse_positive_u32(input: &str) -> Result<u32, ParametricAngleError> {
    if input.is_empty() || !input.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(ParametricAngleError::Invalid);
    }
    let value = input
        .parse::<u32>()
        .map_err(|_| ParametricAngleError::Invalid)?;
    if value == 0 {
        return Err(ParametricAngleError::Invalid);
    }
    Ok(value)
}

fn normalize_fraction(
    numerator: i64,
    denominator: u64,
) -> Result<ParametricAngle, ParametricAngleError> {
    if denominator == 0 || denominator > u32::MAX as u64 {
        return Err(ParametricAngleError::Invalid);
    }
    if numerator == 0 {
        return Ok(ParametricAngle {
            numerator: 0,
            denominator: NonZeroU32::new(1).expect("1 is non-zero"),
        });
    }

    let modulus = 4_i128 * denominator as i128;
    let two_pi = 2_i128 * denominator as i128;
    let mut reduced = (numerator as i128).rem_euclid(modulus);
    if reduced > two_pi {
        reduced -= modulus;
    }
    if reduced == 0 {
        return Ok(ParametricAngle {
            numerator: 0,
            denominator: NonZeroU32::new(1).expect("1 is non-zero"),
        });
    }

    let divisor = gcd(reduced.unsigned_abs() as u64, denominator);
    let normalized_numerator = reduced / divisor as i128;
    let normalized_denominator = denominator / divisor;
    if normalized_numerator < i32::MIN as i128
        || normalized_numerator > i32::MAX as i128
        || normalized_denominator > u32::MAX as u64
    {
        return Err(ParametricAngleError::Invalid);
    }

    Ok(ParametricAngle {
        numerator: normalized_numerator as i32,
        denominator: NonZeroU32::new(normalized_denominator as u32)
            .ok_or(ParametricAngleError::Invalid)?,
    })
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pi_accepts_bare_pi() {
        assert_eq!(ParametricAngle::parse_qni("π").unwrap().label(), "π");
    }

    #[test]
    fn parse_pi_drops_positive_sign() {
        assert_eq!(ParametricAngle::parse_qni("+π").unwrap().label(), "π");
    }

    #[test]
    fn parse_pi_accepts_negative_pi() {
        assert_eq!(ParametricAngle::parse_qni("-π").unwrap().label(), "-π");
    }

    #[test]
    fn parse_pi_accepts_url_separator() {
        assert_eq!(ParametricAngle::parse_qni("π_2").unwrap().label(), "π/2");
    }

    #[test]
    fn parse_pi_reduces_prefix_fraction() {
        assert_eq!(ParametricAngle::parse_qni("4/8π").unwrap().label(), "π/2");
    }

    #[test]
    fn parse_pi_reduces_suffix_fraction() {
        assert_eq!(ParametricAngle::parse_qni("4π/8").unwrap().label(), "π/2");
    }

    #[test]
    fn parse_pi_reduces_negative_fraction() {
        assert_eq!(
            ParametricAngle::parse_qni("-6π/9").unwrap().label(),
            "-2π/3"
        );
    }

    #[test]
    fn parse_pi_reduces_zero_fraction() {
        assert_eq!(ParametricAngle::parse_qni("0π/8").unwrap().label(), "0");
    }

    #[test]
    fn parse_pi_reduces_two_pi_fraction() {
        assert_eq!(ParametricAngle::parse_qni("8π/4").unwrap().label(), "2π");
    }

    #[test]
    fn parse_pi_keeps_two_pi_distinct_from_zero() {
        assert_ne!(
            ParametricAngle::parse_qni("2π").unwrap(),
            ParametricAngle::parse_qni("0").unwrap()
        );
    }

    #[test]
    fn parse_pi_maps_four_pi_to_zero() {
        assert_eq!(ParametricAngle::parse_qni("4π").unwrap().label(), "0");
    }

    #[test]
    fn parse_pi_maps_minus_four_pi_to_zero() {
        assert_eq!(ParametricAngle::parse_qni("-4π").unwrap().label(), "0");
    }

    #[test]
    fn parse_pi_maps_six_pi_to_two_pi() {
        assert_eq!(ParametricAngle::parse_qni("6π").unwrap().label(), "2π");
    }

    #[test]
    fn parse_pi_maps_three_pi_to_minus_pi() {
        assert_eq!(ParametricAngle::parse_qni("3π").unwrap().label(), "-π");
    }

    #[test]
    fn parse_pi_maps_minus_two_pi_to_two_pi() {
        assert_eq!(ParametricAngle::parse_qni("-2π").unwrap().label(), "2π");
    }

    #[test]
    fn parse_pi_rejects_empty_input() {
        assert!(ParametricAngle::parse_qni("").is_err());
    }

    #[test]
    fn parse_pi_rejects_zero_denominator() {
        assert!(ParametricAngle::parse_qni("π/0").is_err());
    }

    #[test]
    fn parse_pi_rejects_plain_number() {
        assert!(ParametricAngle::parse_qni("1.0").is_err());
    }

    #[test]
    fn parse_pi_rejects_alpha_text() {
        assert!(ParametricAngle::parse_qni("abc").is_err());
    }

    #[test]
    fn parse_pi_rejects_decimal_coefficient() {
        assert!(ParametricAngle::parse_qni("0.5π").is_err());
    }

    #[test]
    fn parse_pi_rejects_signed_denominator() {
        assert!(ParametricAngle::parse_qni("π/-2").is_err());
    }

    #[test]
    fn parse_pi_rejects_multiple_pi_symbols() {
        assert!(ParametricAngle::parse_qni("ππ").is_err());
    }

    #[test]
    fn url_label_uses_underscore_separator() {
        assert_eq!(
            ParametricAngle::parse_qni("π/2").unwrap().url_label(),
            "π_2"
        );
    }

    #[test]
    fn parse_user_input_accepts_ascii_pi_with_spaces() {
        assert_eq!(
            ParametricAngle::parse_user_input("pi / 2").unwrap().label(),
            "π/2"
        );
    }

    #[test]
    fn parse_user_input_accepts_bare_fraction_as_pi_coefficient() {
        assert_eq!(
            ParametricAngle::parse_user_input("1/2").unwrap().label(),
            "π/2"
        );
    }

    #[test]
    fn parse_user_input_reduces_bare_fraction() {
        assert_eq!(
            ParametricAngle::parse_user_input("2/4").unwrap().label(),
            "π/2"
        );
    }

    #[test]
    fn parse_user_input_accepts_bare_integer_as_pi_coefficient() {
        assert_eq!(ParametricAngle::parse_user_input("1").unwrap().label(), "π");
    }

    #[test]
    fn parse_user_input_keeps_bare_zero() {
        assert_eq!(ParametricAngle::parse_user_input("0").unwrap().label(), "0");
    }

    #[test]
    fn parse_user_input_accepts_suffix_pi_fraction() {
        assert_eq!(
            ParametricAngle::parse_user_input("1/3π").unwrap().label(),
            "π/3"
        );
    }

    #[test]
    fn parse_user_input_rejects_bare_zero_denominator() {
        assert!(ParametricAngle::parse_user_input("1/0").is_err());
    }

    #[test]
    fn parse_user_input_rejects_bare_signed_denominator() {
        assert!(ParametricAngle::parse_user_input("-2/-4").is_err());
    }

    #[test]
    fn parse_user_input_accepts_uppercase_ascii_pi() {
        assert_eq!(
            ParametricAngle::parse_user_input("PI/2").unwrap().label(),
            "π/2"
        );
    }

    #[test]
    fn radians_returns_reduced_value() {
        assert!(
            (ParametricAngle::parse_qni("4π/8").unwrap().radians_f32()
                - std::f32::consts::FRAC_PI_2)
                .abs()
                < 0.000_001
        );
    }

    #[test]
    fn radians_returns_negative_reduced_value() {
        assert!(
            (ParametricAngle::parse_qni("-6π/9").unwrap().radians_f32()
                - (-2.0 * std::f32::consts::PI / 3.0))
                .abs()
                < 0.000_001
        );
    }

    #[test]
    fn radians_returns_zero_after_four_pi_normalization() {
        assert!(
            ParametricAngle::parse_qni("4π")
                .unwrap()
                .radians_f32()
                .abs()
                < 0.000_001
        );
    }

    #[test]
    fn radians_returns_two_pi_after_six_pi_normalization() {
        assert!(
            (ParametricAngle::parse_qni("6π").unwrap().radians_f32() - 2.0 * std::f32::consts::PI)
                .abs()
                < 0.000_001
        );
    }
}
