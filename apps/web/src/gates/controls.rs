use crate::qubit_bit::QubitBit;

/// 1 つの回路列が持つ制御条件。
///
/// `mask` は制御ワイヤ全体（`Control` と `AntiControl` の両方）のビット、`value` は
/// 値 1 を要求するワイヤ（`Control` のみ）のビット。`AntiControl` は `mask` だけ立て
/// `value` には立てないため、常に `value & !mask == 0`（value のビットは mask の部分集合）
/// という不変条件が保たれる。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ColumnControls {
    mask: u32,
    value: u32,
}

impl ColumnControls {
    /// 制御なしの列。
    pub(crate) const NONE: Self = Self { mask: 0, value: 0 };

    /// 制御ワイヤを 1 本追加する（mask と value の両方を立てる）。
    pub(crate) fn add_control(&mut self, bit: QubitBit) {
        let bit_mask = bit.mask();
        self.mask |= bit_mask;
        self.value |= bit_mask;
    }

    /// 反制御ワイヤを 1 本追加する（mask のみ立てる）。
    pub(crate) fn add_anti_control(&mut self, bit: QubitBit) {
        self.mask |= bit.mask();
    }

    /// 制御ワイヤを 1 本足した新しい制御条件を返す（不変のまま追加）。
    pub(crate) fn with_control(self, bit: QubitBit) -> Self {
        let mut next = self;
        next.add_control(bit);
        next
    }

    /// 指定ビットを mask と value の両方から外した新しい制御条件を返す。
    pub(crate) fn without(self, bit: QubitBit) -> Self {
        let keep = !bit.mask();
        Self {
            mask: self.mask & keep,
            value: self.value & keep,
        }
    }

    /// 値 1 を要求する制御ワイヤだけを残した純制御集合（反制御を落とす）。
    /// CCZ 化で、反制御を除いた Z の制御を作るのに使う。
    pub(crate) fn controls_only(self) -> Self {
        Self {
            mask: self.value,
            value: self.value,
        }
    }

    /// 指定マスク内のビットを mask と value の両方から外した新しい制御条件を返す。
    /// QFT スパン内の制御を除外するのに使う。
    pub(crate) fn restricted_outside(self, span_mask: u32) -> Self {
        let keep = !span_mask;
        Self {
            mask: self.mask & keep,
            value: self.value & keep,
        }
    }

    /// 制御が 1 本もないか（`control_mask == 0` の置き換え）。
    pub(crate) fn is_uncontrolled(self) -> bool {
        self.mask == 0
    }

    /// 値 1 を要求する制御ワイヤの本数（CCZ 化の 2 本以上判定用）。
    pub(crate) fn control_count(self) -> u32 {
        self.value.count_ones()
    }

    /// 値 1 を要求する制御ワイヤのうち最上位のビット（CCZ の Z 対象選択用）。
    pub(crate) fn highest_control(self) -> Option<QubitBit> {
        (self.value != 0).then(|| QubitBit::new(31 - self.value.leading_zeros()))
    }

    /// GPU 境界（GateParams / SimulationOp）へ渡す制御ワイヤ全体のマスク。
    pub(crate) fn mask(self) -> u32 {
        self.mask
    }

    /// GPU 境界へ渡す、値 1 を要求するワイヤのマスク。
    pub(crate) fn value(self) -> u32 {
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::ColumnControls;
    use crate::qubit_bit::QubitBit;

    #[test]
    fn none_is_uncontrolled() {
        assert!(ColumnControls::NONE.is_uncontrolled());
    }

    #[test]
    fn added_control_is_controlled() {
        assert!(!ColumnControls::NONE
            .with_control(QubitBit::new(0))
            .is_uncontrolled());
    }

    #[test]
    fn with_control_does_not_mutate_receiver() {
        let base = ColumnControls::NONE.with_control(QubitBit::new(0));
        let _ = base.with_control(QubitBit::new(2));
        assert_eq!((base.mask(), base.value()), (0b1, 0b1));
    }

    #[test]
    fn controls_only_then_without_drops_anti_and_target() {
        // CCZ composition: keep only Control bits (drop anti-control bit 2),
        // then remove the Z target (bit 1), leaving control bit 0.
        let mut controls = ColumnControls::NONE;
        controls.add_control(QubitBit::new(0));
        controls.add_control(QubitBit::new(1));
        controls.add_anti_control(QubitBit::new(2));
        let cz = controls.controls_only().without(QubitBit::new(1));
        assert_eq!((cz.mask(), cz.value()), (0b1, 0b1));
    }

    #[test]
    fn control_sets_both_mask_and_value() {
        let mut controls = ColumnControls::NONE;
        controls.add_control(QubitBit::new(2));
        assert_eq!((controls.mask(), controls.value()), (0b100, 0b100));
    }

    #[test]
    fn anti_control_sets_mask_only() {
        let mut controls = ColumnControls::NONE;
        controls.add_anti_control(QubitBit::new(2));
        assert_eq!((controls.mask(), controls.value()), (0b100, 0));
    }

    #[test]
    fn value_is_always_subset_of_mask() {
        let mut controls = ColumnControls::NONE;
        controls.add_control(QubitBit::new(0));
        controls.add_anti_control(QubitBit::new(2));
        assert_eq!(controls.value() & !controls.mask(), 0);
    }

    #[test]
    fn control_count_counts_value_bits() {
        let controls = ColumnControls::NONE
            .with_control(QubitBit::new(0))
            .with_control(QubitBit::new(2));
        assert_eq!(controls.control_count(), 2);
    }

    #[test]
    fn highest_control_picks_top_value_bit() {
        let controls = ColumnControls::NONE
            .with_control(QubitBit::new(0))
            .with_control(QubitBit::new(3));
        assert_eq!(controls.highest_control(), Some(QubitBit::new(3)));
    }

    #[test]
    fn highest_control_is_none_without_value_bits() {
        let mut controls = ColumnControls::NONE;
        controls.add_anti_control(QubitBit::new(1));
        assert_eq!(controls.highest_control(), None);
    }

    #[test]
    fn without_clears_bit_from_both() {
        let controls = ColumnControls::NONE
            .with_control(QubitBit::new(1))
            .with_control(QubitBit::new(2))
            .without(QubitBit::new(2));
        assert_eq!((controls.mask(), controls.value()), (0b10, 0b10));
    }

    #[test]
    fn controls_only_drops_anti_controls() {
        let mut controls = ColumnControls::NONE;
        controls.add_control(QubitBit::new(0));
        controls.add_anti_control(QubitBit::new(2));
        assert_eq!(controls.controls_only().mask(), 0b1);
    }

    #[test]
    fn restricted_outside_removes_span_bits() {
        let controls = ColumnControls::NONE
            .with_control(QubitBit::new(0))
            .with_control(QubitBit::new(2));
        assert_eq!(controls.restricted_outside(0b100).value(), 0b1);
    }
}
