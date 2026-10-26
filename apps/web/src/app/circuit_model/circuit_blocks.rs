//! Labelled circuit blocks (qni `circuit-block`).
//!
//! A block groups a contiguous run of circuit columns under a label. In the
//! circuit JSON it is written as an `["{<label>"]` column before its first
//! gate column and a `["}"]` column after its last one; neither marker is a
//! circuit step. Blocks are purely presentational and never reach the
//! simulation plan.
//!
//! Blocks are stored as half-open column ranges `[start, end)` over the same
//! semantic column index the gates use. Every editor operation that shifts
//! gate columns applies the same column transform here, so a block keeps
//! wrapping the gates it was drawn around.

use std::collections::BTreeSet;

use super::CircuitColumnIndex;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CircuitBlock {
    label: String,
    start: usize,
    end: usize,
}

impl CircuitBlock {
    pub(crate) fn label(&self) -> &str {
        &self.label
    }

    pub(crate) fn start(&self) -> CircuitColumnIndex {
        CircuitColumnIndex::new(self.start)
    }

    /// First column after the block (exclusive end).
    pub(crate) fn end(&self) -> CircuitColumnIndex {
        CircuitColumnIndex::new(self.end)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CircuitBlockError {
    /// `start >= end`: a block must contain at least one column.
    Empty,
    /// The block starts before the previous block ends. Blocks never nest or
    /// overlap.
    Overlap,
    /// No block has the given index.
    Missing,
}

/// Ordered, non-overlapping, non-empty circuit blocks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct CircuitBlocks {
    blocks: Vec<CircuitBlock>,
}

impl CircuitBlocks {
    /// Append a block after every existing block.
    pub(crate) fn push(
        &mut self,
        label: String,
        start: CircuitColumnIndex,
        end: CircuitColumnIndex,
    ) -> Result<(), CircuitBlockError> {
        let (start, end) = (start.as_usize(), end.as_usize());
        if start >= end {
            return Err(CircuitBlockError::Empty);
        }
        if self.blocks.last().is_some_and(|last| start < last.end) {
            return Err(CircuitBlockError::Overlap);
        }
        self.blocks.push(CircuitBlock { label, start, end });
        Ok(())
    }

    /// Insert a block anywhere between the existing ones and return its index.
    pub(crate) fn insert(
        &mut self,
        label: String,
        start: CircuitColumnIndex,
        end: CircuitColumnIndex,
    ) -> Result<usize, CircuitBlockError> {
        let (start, end) = (start.as_usize(), end.as_usize());
        if start >= end {
            return Err(CircuitBlockError::Empty);
        }
        if !self.is_free(start, end) {
            return Err(CircuitBlockError::Overlap);
        }
        let index = self.blocks.partition_point(|block| block.end <= start);
        self.blocks
            .insert(index, CircuitBlock { label, start, end });
        Ok(index)
    }

    /// Whether no block covers any of the columns `[start, end)`.
    pub(crate) fn is_free(&self, start: usize, end: usize) -> bool {
        self.blocks
            .iter()
            .all(|block| block.end <= start || end <= block.start)
    }

    pub(crate) fn set_label(
        &mut self,
        index: usize,
        label: String,
    ) -> Result<(), CircuitBlockError> {
        let block = self
            .blocks
            .get_mut(index)
            .ok_or(CircuitBlockError::Missing)?;
        block.label = label;
        Ok(())
    }

    /// Change the column range of a block without passing its neighbours.
    pub(crate) fn resize(
        &mut self,
        index: usize,
        start: CircuitColumnIndex,
        end: CircuitColumnIndex,
    ) -> Result<(), CircuitBlockError> {
        let (start, end) = (start.as_usize(), end.as_usize());
        if index >= self.blocks.len() {
            return Err(CircuitBlockError::Missing);
        }
        if start >= end {
            return Err(CircuitBlockError::Empty);
        }
        let previous_end = index
            .checked_sub(1)
            .map_or(0, |previous| self.blocks[previous].end);
        let next_start = self
            .blocks
            .get(index + 1)
            .map_or(usize::MAX, |next| next.start);
        if start < previous_end || end > next_start {
            return Err(CircuitBlockError::Overlap);
        }
        let block = &mut self.blocks[index];
        block.start = start;
        block.end = end;
        Ok(())
    }

    /// Move a block so it starts at `start`, keeping its width, and return its
    /// new index. The block may jump over a neighbour but never overlap one.
    pub(crate) fn move_to(
        &mut self,
        index: usize,
        start: CircuitColumnIndex,
    ) -> Result<usize, CircuitBlockError> {
        let block = self.remove(index).ok_or(CircuitBlockError::Missing)?;
        let end = start.checked_add(block.end - block.start);
        let moved = end
            .ok_or(CircuitBlockError::Overlap)
            .and_then(|end| self.insert(block.label.clone(), start, end));
        if moved.is_err() {
            self.blocks.insert(index, block);
        }
        moved
    }

    pub(crate) fn remove(&mut self, index: usize) -> Option<CircuitBlock> {
        (index < self.blocks.len()).then(|| self.blocks.remove(index))
    }

    pub(crate) fn get(&self, index: usize) -> Option<&CircuitBlock> {
        self.blocks.get(index)
    }

    pub(crate) fn iter(&self) -> impl DoubleEndedIterator<Item = &CircuitBlock> {
        self.blocks.iter()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    pub(crate) fn clear(&mut self) {
        self.blocks.clear();
    }

    /// Exclusive end of the last block, or 0 when there are no blocks.
    pub(crate) fn column_count(&self) -> usize {
        self.blocks.last().map_or(0, |block| block.end)
    }

    /// `width` new columns were inserted before column `at`. An insertion
    /// strictly inside a block widens it; one at or before a block's first
    /// column moves the whole block right.
    pub(crate) fn insert_columns(&mut self, at: CircuitColumnIndex, width: usize) {
        let at = at.as_usize();
        for block in &mut self.blocks {
            if at <= block.start {
                block.start = block.start.saturating_add(width);
            }
            if at < block.end {
                block.end = block.end.saturating_add(width);
            }
        }
    }

    /// The gate starting at `column` grew `delta` columns wider. The new
    /// columns belong to whatever block holds `column`; later blocks move
    /// right.
    pub(crate) fn widen_column(&mut self, column: CircuitColumnIndex, delta: usize) {
        let column = column.as_usize();
        for block in &mut self.blocks {
            if block.start > column {
                block.start = block.start.saturating_add(delta);
            }
            if block.end > column {
                block.end = block.end.saturating_add(delta);
            }
        }
    }

    /// Columns `[at, at + width)` were removed and later columns shifted left.
    /// Blocks left without any column disappear.
    pub(crate) fn remove_columns(&mut self, at: CircuitColumnIndex, width: usize) {
        let at = at.as_usize();
        let removed_end = at.saturating_add(width);
        let remap = |column: usize| column - column.clamp(at, removed_end).saturating_sub(at);
        self.remap(remap);
    }

    /// Empty columns were collapsed so that only `occupied` columns remain,
    /// renumbered densely from 0. Blocks left without any occupied column
    /// disappear (qni drops empty blocks the same way on `removeEmptySteps`).
    pub(crate) fn compact_to(&mut self, occupied: &BTreeSet<usize>) {
        self.remap(|column| occupied.range(..column).count());
    }

    fn remap(&mut self, remap: impl Fn(usize) -> usize) {
        for block in &mut self.blocks {
            block.start = remap(block.start);
            block.end = remap(block.end);
        }
        self.blocks.retain(|block| block.start < block.end);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn col(value: usize) -> CircuitColumnIndex {
        CircuitColumnIndex::new(value)
    }

    fn blocks(ranges: &[(usize, usize)]) -> CircuitBlocks {
        let mut blocks = CircuitBlocks::default();
        for (index, &(start, end)) in ranges.iter().enumerate() {
            blocks
                .push(format!("b{index}"), col(start), col(end))
                .expect("test block ranges are valid");
        }
        blocks
    }

    fn ranges(blocks: &CircuitBlocks) -> Vec<(usize, usize)> {
        blocks
            .iter()
            .map(|block| (block.start().as_usize(), block.end().as_usize()))
            .collect()
    }

    #[test]
    fn push_rejects_empty_range() {
        assert_eq!(
            CircuitBlocks::default().push("a".to_owned(), col(2), col(2)),
            Err(CircuitBlockError::Empty)
        );
    }

    #[test]
    fn push_rejects_overlapping_range() {
        let mut blocks = blocks(&[(0, 3)]);

        assert_eq!(
            blocks.push("b".to_owned(), col(2), col(4)),
            Err(CircuitBlockError::Overlap)
        );
    }

    #[test]
    fn push_accepts_adjacent_range() {
        let mut blocks = blocks(&[(0, 3)]);

        assert_eq!(blocks.push("b".to_owned(), col(3), col(4)), Ok(()));
    }

    #[test]
    fn insert_places_block_between_existing_blocks() {
        let mut blocks = blocks(&[(0, 1), (4, 5)]);
        blocks
            .insert("new".to_owned(), col(2), col(3))
            .expect("the gap is free");

        assert_eq!(ranges(&blocks), vec![(0, 1), (2, 3), (4, 5)]);
    }

    #[test]
    fn insert_returns_the_new_block_index() {
        let mut blocks = blocks(&[(0, 1), (4, 5)]);

        assert_eq!(blocks.insert("new".to_owned(), col(2), col(3)), Ok(1));
    }

    #[test]
    fn insert_rejects_overlap_with_a_later_block() {
        let mut blocks = blocks(&[(0, 1), (4, 6)]);

        assert_eq!(
            blocks.insert("new".to_owned(), col(3), col(5)),
            Err(CircuitBlockError::Overlap)
        );
    }

    #[test]
    fn insert_rejects_overlap_with_an_earlier_block() {
        let mut blocks = blocks(&[(0, 3)]);

        assert_eq!(
            blocks.insert("new".to_owned(), col(2), col(4)),
            Err(CircuitBlockError::Overlap)
        );
    }

    #[test]
    fn insert_rejects_empty_range() {
        assert_eq!(
            CircuitBlocks::default().insert("new".to_owned(), col(1), col(1)),
            Err(CircuitBlockError::Empty)
        );
    }

    #[test]
    fn insert_accepts_blocks_adjacent_on_both_sides() {
        let mut blocks = blocks(&[(0, 2), (3, 5)]);

        assert_eq!(blocks.insert("new".to_owned(), col(2), col(3)), Ok(1));
    }

    #[test]
    fn set_label_replaces_the_label() {
        let mut blocks = blocks(&[(0, 1)]);
        blocks
            .set_label(0, "量子もつれ".to_owned())
            .expect("block 0 exists");

        assert_eq!(blocks.get(0).map(CircuitBlock::label), Some("量子もつれ"));
    }

    #[test]
    fn set_label_rejects_missing_block() {
        assert_eq!(
            CircuitBlocks::default().set_label(0, "a".to_owned()),
            Err(CircuitBlockError::Missing)
        );
    }

    #[test]
    fn resize_changes_the_range() {
        let mut blocks = blocks(&[(1, 2)]);
        blocks.resize(0, col(0), col(3)).expect("no neighbours");

        assert_eq!(ranges(&blocks), vec![(0, 3)]);
    }

    #[test]
    fn resize_stops_at_the_next_block() {
        let mut blocks = blocks(&[(0, 1), (2, 3)]);

        assert_eq!(
            blocks.resize(0, col(0), col(3)),
            Err(CircuitBlockError::Overlap)
        );
    }

    #[test]
    fn resize_stops_at_the_previous_block() {
        let mut blocks = blocks(&[(0, 2), (2, 3)]);

        assert_eq!(
            blocks.resize(1, col(1), col(3)),
            Err(CircuitBlockError::Overlap)
        );
    }

    #[test]
    fn resize_rejects_less_than_one_column() {
        let mut blocks = blocks(&[(1, 3)]);

        assert_eq!(
            blocks.resize(0, col(3), col(3)),
            Err(CircuitBlockError::Empty)
        );
    }

    #[test]
    fn move_to_keeps_the_width() {
        let mut blocks = blocks(&[(0, 2)]);
        blocks.move_to(0, col(3)).expect("no neighbours");

        assert_eq!(ranges(&blocks), vec![(3, 5)]);
    }

    #[test]
    fn move_to_past_a_neighbour_returns_the_new_index() {
        let mut blocks = blocks(&[(0, 1), (1, 2)]);

        assert_eq!(blocks.move_to(0, col(4)), Ok(1));
    }

    #[test]
    fn move_to_rejects_overlap() {
        let mut blocks = blocks(&[(0, 1), (2, 4)]);

        assert_eq!(blocks.move_to(0, col(3)), Err(CircuitBlockError::Overlap));
    }

    #[test]
    fn rejected_move_keeps_the_block_in_place() {
        let mut blocks = blocks(&[(0, 1), (2, 4)]);
        let _ = blocks.move_to(0, col(3));

        assert_eq!(ranges(&blocks), vec![(0, 1), (2, 4)]);
    }

    #[test]
    fn remove_drops_only_that_block() {
        let mut blocks = blocks(&[(0, 1), (2, 3)]);
        blocks.remove(0);

        assert_eq!(ranges(&blocks), vec![(2, 3)]);
    }

    #[test]
    fn column_count_is_last_block_end() {
        assert_eq!(blocks(&[(0, 1), (2, 5)]).column_count(), 5);
    }

    #[test]
    fn insert_before_block_moves_it_right() {
        let mut blocks = blocks(&[(2, 4)]);
        blocks.insert_columns(col(1), 1);

        assert_eq!(ranges(&blocks), vec![(3, 5)]);
    }

    #[test]
    fn insert_at_block_start_moves_it_right() {
        let mut blocks = blocks(&[(2, 4)]);
        blocks.insert_columns(col(2), 1);

        assert_eq!(ranges(&blocks), vec![(3, 5)]);
    }

    #[test]
    fn insert_inside_block_widens_it() {
        let mut blocks = blocks(&[(2, 4)]);
        blocks.insert_columns(col(3), 2);

        assert_eq!(ranges(&blocks), vec![(2, 6)]);
    }

    #[test]
    fn insert_at_block_end_leaves_it_alone() {
        let mut blocks = blocks(&[(2, 4)]);
        blocks.insert_columns(col(4), 1);

        assert_eq!(ranges(&blocks), vec![(2, 4)]);
    }

    #[test]
    fn widen_last_column_of_block_grows_block() {
        let mut blocks = blocks(&[(2, 4), (4, 5)]);
        blocks.widen_column(col(3), 2);

        assert_eq!(ranges(&blocks), vec![(2, 6), (6, 7)]);
    }

    #[test]
    fn widen_column_before_block_moves_it_right() {
        let mut blocks = blocks(&[(2, 4)]);
        blocks.widen_column(col(0), 1);

        assert_eq!(ranges(&blocks), vec![(3, 5)]);
    }

    #[test]
    fn remove_column_inside_block_shrinks_it() {
        let mut blocks = blocks(&[(2, 5)]);
        blocks.remove_columns(col(3), 1);

        assert_eq!(ranges(&blocks), vec![(2, 4)]);
    }

    #[test]
    fn remove_column_before_block_moves_it_left() {
        let mut blocks = blocks(&[(2, 5)]);
        blocks.remove_columns(col(0), 2);

        assert_eq!(ranges(&blocks), vec![(0, 3)]);
    }

    #[test]
    fn remove_every_block_column_drops_block() {
        let mut blocks = blocks(&[(1, 2), (2, 4)]);
        blocks.remove_columns(col(1), 1);

        assert_eq!(ranges(&blocks), vec![(1, 3)]);
    }

    #[test]
    fn remove_columns_then_shrinking_widen_round_trips() {
        let mut blocks = blocks(&[(2, 4), (5, 6)]);
        blocks.widen_column(col(3), 2);
        blocks.remove_columns(col(4), 2);

        assert_eq!(ranges(&blocks), vec![(2, 4), (5, 6)]);
    }

    #[test]
    fn compact_renumbers_occupied_columns() {
        let mut blocks = blocks(&[(1, 4), (6, 8)]);
        blocks.compact_to(&BTreeSet::from([0, 2, 3, 7]));

        assert_eq!(ranges(&blocks), vec![(1, 3), (3, 4)]);
    }

    #[test]
    fn compact_drops_block_without_occupied_columns() {
        let mut blocks = blocks(&[(1, 3)]);
        blocks.compact_to(&BTreeSet::from([0, 4]));

        assert!(blocks.is_empty());
    }
}
