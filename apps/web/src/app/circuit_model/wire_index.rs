use crate::qubit_bit::QubitBit;
use crate::qubit_count::QubitCount;

/// 回路エディタのワイヤ番号。画面上端のワイヤを 0 として下へ増える。
///
/// GPU 側のビット位置 [`QubitBit`] と並びが一致し（上ワイヤ q0 = 最下位ビット LSB、
/// Quirk/標準のリトルエンディアン）、`to_qubit_bit` は wire をそのままビット位置にする恒等写像。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub(crate) struct WireIndex {
    value: usize,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WireIndexError {
    Negative,
    Overflow,
}

impl WireIndex {
    pub(crate) const ZERO: Self = Self { value: 0 };

    pub(crate) const fn new(value: usize) -> Self {
        Self { value }
    }

    #[allow(dead_code)]
    pub(crate) fn try_from_i64(value: i64) -> Result<Self, WireIndexError> {
        let value = usize::try_from(value).map_err(|_| {
            if value < 0 {
                WireIndexError::Negative
            } else {
                WireIndexError::Overflow
            }
        })?;
        Ok(Self::new(value))
    }

    pub(crate) const fn as_usize(self) -> usize {
        self.value
    }

    /// 量子ビット数の範囲内か（`wire < qubits` ガードの置き換え）。
    pub(crate) fn is_within(self, qubits: QubitCount) -> bool {
        self.value < qubits.get()
    }

    /// ビット位置へ変換する。範囲外なら `None`。
    /// 上ワイヤ q0 = bit0(LSB) の恒等写像（wire をそのままビット位置にする。Quirk/標準のリトルエンディアン）。
    pub(crate) fn to_qubit_bit(self, qubits: QubitCount) -> Option<QubitBit> {
        let qubits = qubits.get();
        (self.value < qubits).then(|| QubitBit::new(self.value as u32))
    }

    /// 下方向へ `delta` 本ずらしたワイヤ（スパン内・QFT 展開のワイヤ走査用）。
    pub(crate) fn offset(self, delta: usize) -> Self {
        Self::new(self.value + delta)
    }
}

#[cfg(test)]
mod tests {
    use super::{WireIndex, WireIndexError};
    use crate::qubit_bit::QubitBit;
    use crate::qubit_count::QubitCount;

    fn qubits(value: usize) -> QubitCount {
        QubitCount::try_new(value).expect("test qubit count must be non-zero")
    }

    #[test]
    fn zero_is_top_wire() {
        assert_eq!(WireIndex::ZERO.as_usize(), 0);
    }

    #[test]
    fn new_keeps_value() {
        assert_eq!(WireIndex::new(2).as_usize(), 2);
    }

    #[test]
    fn try_from_i64_rejects_negative() {
        assert_eq!(WireIndex::try_from_i64(-1), Err(WireIndexError::Negative));
    }

    #[test]
    fn equal_wires_compare_by_value() {
        assert_eq!(WireIndex::new(1), WireIndex::new(1));
    }

    #[test]
    fn different_wires_are_not_equal() {
        assert_ne!(WireIndex::new(1), WireIndex::new(2));
    }

    #[test]
    fn wire_order_matches_numeric_order() {
        assert!(WireIndex::new(1) < WireIndex::new(2));
    }

    #[test]
    fn top_wire_maps_to_least_significant_bit() {
        assert_eq!(
            WireIndex::ZERO.to_qubit_bit(qubits(3)),
            Some(QubitBit::new(0))
        );
    }

    #[test]
    fn bottom_wire_maps_to_most_significant_bit() {
        assert_eq!(
            WireIndex::new(2).to_qubit_bit(qubits(3)),
            Some(QubitBit::new(2))
        );
    }

    #[test]
    fn out_of_range_wire_has_no_bit() {
        assert_eq!(WireIndex::new(3).to_qubit_bit(qubits(3)), None);
    }

    #[test]
    fn out_of_range_wire_is_not_within() {
        assert!(!WireIndex::new(3).is_within(qubits(3)));
    }

    #[test]
    fn offset_moves_down() {
        assert_eq!(WireIndex::ZERO.offset(2).as_usize(), 2);
    }
}
