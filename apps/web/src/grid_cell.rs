//! 桁数 `cols` はグリッドの寸法であってセル位置の一部ではないため、`GridCell` は桁・行の組
//! だけを持ち、`cols` は往復変換のたびに引数で受け取る。呼び出し側は `cols >= 1` を保証する
//! （グリッド寸法が `max(1)` 済み、または `1 << span` で必ず 1 以上）。

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct GridCell {
    col: usize,
    row: usize,
}

impl GridCell {
    pub(crate) const fn new(col: usize, row: usize) -> Self {
        Self { col, row }
    }

    pub(crate) const fn from_index(index: usize, cols: usize) -> Self {
        Self {
            col: index % cols,
            row: index / cols,
        }
    }

    pub(crate) const fn col(self) -> usize {
        self.col
    }

    pub(crate) const fn row(self) -> usize {
        self.row
    }

    pub(crate) const fn to_index(self, cols: usize) -> usize {
        self.row * cols + self.col
    }
}

#[cfg(test)]
mod tests {
    use super::GridCell;

    #[test]
    fn new_keeps_col() {
        assert_eq!(GridCell::new(1, 2).col(), 1);
    }

    #[test]
    fn new_keeps_row() {
        assert_eq!(GridCell::new(1, 2).row(), 2);
    }

    #[test]
    fn from_index_recovers_col() {
        assert_eq!(GridCell::from_index(7, 3).col(), 1);
    }

    #[test]
    fn from_index_recovers_row() {
        assert_eq!(GridCell::from_index(7, 3).row(), 2);
    }

    #[test]
    fn to_index_folds_row_major() {
        assert_eq!(GridCell::new(1, 2).to_index(3), 7);
    }

    #[test]
    fn round_trip_returns_original_index() {
        assert_eq!(GridCell::from_index(11, 4).to_index(4), 11);
    }

    #[test]
    fn from_index_zero_is_origin_col() {
        assert_eq!(GridCell::from_index(0, 3).col(), 0);
    }

    #[test]
    fn from_index_zero_is_origin_row() {
        assert_eq!(GridCell::from_index(0, 3).row(), 0);
    }

    #[test]
    fn equal_cells_compare_by_value() {
        assert_eq!(GridCell::new(1, 2), GridCell::new(1, 2));
    }

    #[test]
    fn different_cells_are_not_equal() {
        assert_ne!(GridCell::new(1, 2), GridCell::new(2, 1));
    }
}
