use crate::gates::ColumnControls;

/// ローカル算術ゲートの対象範囲。状態の添字や振幅は計算しない。
/// 呼び出し側で有効なワイヤとクランプ済みの範囲を渡すことを前提とする。
/// ローカル上限は 16 ビットで、`1 <= span <= n - wire` を満たす。
/// 変更時は `bash apps/web/scripts/verify-kani.sh` で形式検証する。
pub(crate) struct ArithmeticField {
    shift: u32,
    mask: u32,
}

impl ArithmeticField {
    pub(crate) fn new(wire: u32, span: u32) -> Self {
        Self {
            shift: wire,
            mask: (1u32 << span) - 1,
        }
    }

    pub(crate) fn shift(&self) -> u32 {
        self.shift
    }

    pub(crate) fn mask(&self) -> u32 {
        self.mask
    }

    pub(crate) fn amount(increment: bool) -> u32 {
        if increment {
            1
        } else {
            u32::MAX
        }
    }

    pub(crate) fn external_controls(&self, controls: ColumnControls) -> ColumnControls {
        controls.restricted_outside(self.mask << self.shift)
    }
}

#[cfg(test)]
mod tests {
    use super::ArithmeticField;
    use crate::gates::ColumnControls;
    use crate::qubit_bit::QubitBit;

    #[test]
    fn field_keeps_shift_and_low_mask() {
        let field = ArithmeticField::new(3, 4);
        assert_eq!((field.shift(), field.mask()), (3, 15));
    }

    #[test]
    fn field_reaches_local_upper_boundary() {
        let field = ArithmeticField::new(0, 16);
        assert_eq!(field.mask(), 65535);
    }

    #[test]
    fn amounts_match_shader_wrapping_arithmetic() {
        assert_eq!(
            (
                ArithmeticField::amount(true),
                ArithmeticField::amount(false)
            ),
            (1, u32::MAX)
        );
    }

    #[test]
    fn external_controls_remove_only_shifted_field() {
        let controls = ColumnControls::NONE
            .with_control(QubitBit::new(1))
            .with_control(QubitBit::new(3))
            .with_control(QubitBit::new(5));
        let outside = ArithmeticField::new(2, 3).external_controls(controls);
        assert_eq!((outside.mask(), outside.value()), (0b100010, 0b100010));
    }
}
