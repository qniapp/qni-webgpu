use eframe::egui;

use crate::constants::{PALETTE_GAP, PALETTE_ROW_GAP, PALETTE_SIZE};
use crate::gates::{PALETTE_GATES, PALETTE_ROW1_COUNT};

#[derive(Clone, Copy, Debug)]
pub(crate) struct PaletteLayout {
    pub(crate) total_width: f32,
    pub(crate) total_height: f32,
}

fn palette_row_width(count: usize) -> f32 {
    if count == 0 {
        0.0
    } else {
        count as f32 * PALETTE_SIZE + (count - 1) as f32 * PALETTE_GAP
    }
}

pub(crate) fn palette_layout() -> PaletteLayout {
    let row1_count = PALETTE_ROW1_COUNT;
    let row2_count = PALETTE_GATES.len().saturating_sub(row1_count);
    let row1_width = palette_row_width(row1_count);
    let row2_width = palette_row_width(row2_count);
    PaletteLayout {
        total_width: row1_width.max(row2_width),
        total_height: 2.0 * PALETTE_SIZE + PALETTE_ROW_GAP,
    }
}

fn palette_row_for_index(index: usize) -> Option<(usize, usize)> {
    if index < PALETTE_ROW1_COUNT {
        Some((0, index))
    } else if index < PALETTE_GATES.len() {
        Some((1, index - PALETTE_ROW1_COUNT))
    } else {
        None
    }
}

/// Returns gate top-left position relative to the palette panel top-left.
/// Both rows are left-aligned, matching qni's `flex flex-row` layout.
pub(crate) fn palette_gate_local_pos(index: usize, _layout: &PaletteLayout) -> Option<egui::Pos2> {
    let (row, col) = palette_row_for_index(index)?;
    let x = col as f32 * (PALETTE_SIZE + PALETTE_GAP);
    let y = row as f32 * (PALETTE_SIZE + PALETTE_ROW_GAP);
    Some(egui::pos2(x, y))
}

/// Maps a cursor position (relative to the palette panel top-left) to a flat
/// gate index. Returns `None` outside any gate cell.
pub(crate) fn palette_hit_test(local_pos: egui::Pos2, _layout: &PaletteLayout) -> Option<usize> {
    if local_pos.y < 0.0 || local_pos.x < 0.0 {
        return None;
    }
    let row = if local_pos.y <= PALETTE_SIZE {
        0
    } else if local_pos.y >= PALETTE_SIZE + PALETTE_ROW_GAP
        && local_pos.y <= 2.0 * PALETTE_SIZE + PALETTE_ROW_GAP
    {
        1
    } else {
        return None;
    };
    let row_count = if row == 0 {
        PALETTE_ROW1_COUNT
    } else {
        PALETTE_GATES.len() - PALETTE_ROW1_COUNT
    };
    let col_index = (local_pos.x / (PALETTE_SIZE + PALETTE_GAP)).floor() as i32;
    if col_index < 0 || (col_index as usize) >= row_count {
        return None;
    }
    let col_offset = local_pos.x - col_index as f32 * (PALETTE_SIZE + PALETTE_GAP);
    if col_offset > PALETTE_SIZE {
        return None;
    }
    let flat_index = if row == 0 {
        col_index as usize
    } else {
        PALETTE_ROW1_COUNT + col_index as usize
    };
    Some(flat_index)
}
