/// GPU の量子状態計算が使う MSB 始まりの量子ビット番号。
///
/// エディタのワイヤ番号（上から下へ 0 始まり）とは並びが逆で、最上段ワイヤが
/// 最上位ビットに対応する。値は `qubits - 1 - wire` 変換、制御マスクの最上位ビット抽出、
/// 表示ブロックのスパン下端から生まれ、ビット位置は量子ビット数未満（量子ビット数 ≤ 32 なので 0..=31）に収まる。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub(crate) struct QubitBit {
    value: u32,
}

impl QubitBit {
    pub(crate) const fn new(value: u32) -> Self {
        Self { value }
    }

    pub(crate) const fn as_u32(self) -> u32 {
        self.value
    }

    /// このビット位置だけを立てたビットマスク。
    ///
    /// ビット位置が 32 未満であることを前提に `1u32 << value` を返す。この前提は
    /// 生成元の範囲（量子ビット数 ≤ 32）から満たされる。
    pub(crate) const fn mask(self) -> u32 {
        1u32 << self.value
    }
}

#[cfg(test)]
mod tests {
    use super::QubitBit;

    #[test]
    fn keeps_bit_position() {
        assert_eq!(QubitBit::new(2).as_u32(), 2);
    }

    #[test]
    fn keeps_bit_zero() {
        assert_eq!(QubitBit::new(0).as_u32(), 0);
    }

    #[test]
    fn keeps_top_bit() {
        assert_eq!(QubitBit::new(31).as_u32(), 31);
    }

    #[test]
    fn mask_of_bit_zero_is_lowest_bit() {
        assert_eq!(QubitBit::new(0).mask(), 0b1);
    }

    #[test]
    fn mask_of_bit_two_sets_third_bit() {
        assert_eq!(QubitBit::new(2).mask(), 0b100);
    }

    #[test]
    fn mask_of_top_bit_sets_highest_bit() {
        assert_eq!(QubitBit::new(31).mask(), 1u32 << 31);
    }

    #[test]
    fn equal_bits_compare_by_value() {
        assert_eq!(QubitBit::new(1), QubitBit::new(1));
    }

    #[test]
    fn different_bits_are_not_equal() {
        assert_ne!(QubitBit::new(1), QubitBit::new(2));
    }

    #[test]
    fn bit_order_matches_numeric_order() {
        assert!(QubitBit::new(1) < QubitBit::new(2));
    }
}
