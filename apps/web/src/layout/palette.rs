use eframe::egui;

use crate::constants::{
    PALETTE_DISPLAY_COLUMNS, PALETTE_DISPLAY_ROWS, PALETTE_GAP, PALETTE_ROW_GAP,
    PALETTE_SECTION_GAP, PALETTE_SEPARATOR_WIDTH, PALETTE_SIZE,
};
use crate::gates::{
    PALETTE_DISPLAY_GATES, PALETTE_DISPLAY_INDICES, PALETTE_GATES_ROW2, PALETTE_GATES_ROW2_INDICES,
    PALETTE_GATE_COUNT, PALETTE_ROW1_COUNT,
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct PaletteLayout {
    pub(crate) total_width: f32,
    pub(crate) total_height: f32,
    pub(crate) gates_width: f32,
    pub(crate) separator_x: f32,
    pub(crate) display_x: f32,
    pub(crate) display_width: f32,
}

fn palette_row_width(count: usize) -> f32 {
    if count == 0 {
        0.0
    } else {
        count as f32 * PALETTE_SIZE + (count - 1) as f32 * PALETTE_GAP
    }
}

fn palette_grid_width(columns: usize) -> f32 {
    palette_row_width(columns)
}

pub(crate) fn palette_layout() -> PaletteLayout {
    let gates_row1_width = palette_row_width(PALETTE_ROW1_COUNT);
    let gates_row2_width = palette_row_width(PALETTE_GATES_ROW2.len());
    let gates_width = gates_row1_width.max(gates_row2_width);
    let separator_x = gates_width + PALETTE_SECTION_GAP;
    let display_x = separator_x + PALETTE_SEPARATOR_WIDTH + PALETTE_SECTION_GAP;
    let display_width = palette_grid_width(PALETTE_DISPLAY_COLUMNS);
    PaletteLayout {
        total_width: display_x + display_width,
        total_height: PALETTE_DISPLAY_ROWS as f32 * PALETTE_SIZE
            + (PALETTE_DISPLAY_ROWS.saturating_sub(1)) as f32 * PALETTE_ROW_GAP,
        gates_width,
        separator_x,
        display_x,
        display_width,
    }
}

pub(crate) fn palette_start_x(width: f32, layout: &PaletteLayout) -> f32 {
    // Odd-width viewports would otherwise place the palette on a half pixel,
    // making PNG glyph textures look thinner than the integer-aligned circuit.
    (width / 2.0 - layout.total_width / 2.0).round()
}

fn palette_gates_local_pos(index: usize) -> Option<egui::Pos2> {
    if index < PALETTE_ROW1_COUNT {
        return Some(egui::pos2(index as f32 * (PALETTE_SIZE + PALETTE_GAP), 0.0));
    }
    let col = PALETTE_GATES_ROW2_INDICES
        .iter()
        .position(|candidate| *candidate == index)?;
    Some(egui::pos2(
        col as f32 * (PALETTE_SIZE + PALETTE_GAP),
        PALETTE_SIZE + PALETTE_ROW_GAP,
    ))
}

fn palette_display_local_pos(index: usize, layout: &PaletteLayout) -> Option<egui::Pos2> {
    let slot = PALETTE_DISPLAY_INDICES
        .iter()
        .position(|candidate| *candidate == index)?;
    let row = slot / PALETTE_DISPLAY_COLUMNS;
    let col = slot % PALETTE_DISPLAY_COLUMNS;
    Some(egui::pos2(
        layout.display_x + col as f32 * (PALETTE_SIZE + PALETTE_GAP),
        row as f32 * (PALETTE_SIZE + PALETTE_ROW_GAP),
    ))
}

/// Returns gate top-left position relative to the palette panel top-left.
/// Gates stay in the left section; Display widgets occupy a 2×2 grid on the
/// right, leaving the bottom-right slot intentionally empty and non-hoverable.
pub(crate) fn palette_gate_local_pos(index: usize, layout: &PaletteLayout) -> Option<egui::Pos2> {
    if index >= PALETTE_GATE_COUNT {
        return None;
    }
    palette_gates_local_pos(index).or_else(|| palette_display_local_pos(index, layout))
}

fn row_from_y(y: f32) -> Option<usize> {
    if (0.0..=PALETTE_SIZE).contains(&y) {
        Some(0)
    } else if (PALETTE_SIZE + PALETTE_ROW_GAP..=2.0 * PALETTE_SIZE + PALETTE_ROW_GAP).contains(&y) {
        Some(1)
    } else {
        None
    }
}

fn col_from_x(x: f32) -> Option<usize> {
    if x < 0.0 {
        return None;
    }
    let col = (x / (PALETTE_SIZE + PALETTE_GAP)).floor() as usize;
    let col_offset = x - col as f32 * (PALETTE_SIZE + PALETTE_GAP);
    if col_offset > PALETTE_SIZE {
        None
    } else {
        Some(col)
    }
}

fn hit_test_gates(local_pos: egui::Pos2) -> Option<usize> {
    let row = row_from_y(local_pos.y)?;
    let col = col_from_x(local_pos.x)?;
    if row == 0 {
        (col < PALETTE_ROW1_COUNT).then_some(col)
    } else {
        PALETTE_GATES_ROW2_INDICES.get(col).copied()
    }
}

fn hit_test_display(local_pos: egui::Pos2, layout: &PaletteLayout) -> Option<usize> {
    let row = row_from_y(local_pos.y)?;
    let col = col_from_x(local_pos.x - layout.display_x)?;
    if col >= PALETTE_DISPLAY_COLUMNS {
        return None;
    }
    let slot = row * PALETTE_DISPLAY_COLUMNS + col;
    if slot >= PALETTE_DISPLAY_GATES.len() {
        return None;
    }
    PALETTE_DISPLAY_INDICES.get(slot).copied()
}

/// Maps a cursor position (relative to the palette panel top-left) to a flat
/// gate index. Returns `None` outside any gate cell, including the separator,
/// inter-section gaps, and the Display section's empty bottom-right slot.
pub(crate) fn palette_hit_test(local_pos: egui::Pos2, layout: &PaletteLayout) -> Option<usize> {
    if local_pos.y < 0.0 || local_pos.x < 0.0 {
        return None;
    }
    if local_pos.x <= layout.gates_width {
        return hit_test_gates(local_pos);
    }
    if (layout.display_x..=layout.display_x + layout.display_width).contains(&local_pos.x) {
        return hit_test_display(local_pos, layout);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_hit_test_finds_gates_section_cell() {
        let layout = palette_layout();
        let local = palette_gate_local_pos(14, &layout).unwrap() + egui::vec2(20.0, 20.0);

        assert_eq!(palette_hit_test(local, &layout), Some(14));
    }

    #[test]
    fn palette_hit_test_finds_display_section_cell() {
        let layout = palette_layout();
        let local = palette_gate_local_pos(20, &layout).unwrap() + egui::vec2(20.0, 20.0);

        assert_eq!(palette_hit_test(local, &layout), Some(20));
    }

    #[test]
    fn palette_hit_test_ignores_display_empty_slot() {
        let layout = palette_layout();
        let local = egui::pos2(
            layout.display_x + PALETTE_SIZE + PALETTE_GAP + PALETTE_SIZE / 2.0,
            PALETTE_SIZE + PALETTE_ROW_GAP + PALETTE_SIZE / 2.0,
        );

        assert_eq!(palette_hit_test(local, &layout), None);
    }

    #[test]
    fn palette_hit_test_round_trips_all_flat_indices() {
        let layout = palette_layout();
        let all_round_trip = (0..PALETTE_GATE_COUNT).all(|index| {
            let Some(local) = palette_gate_local_pos(index, &layout) else {
                return false;
            };
            palette_hit_test(
                local + egui::vec2(PALETTE_SIZE / 2.0, PALETTE_SIZE / 2.0),
                &layout,
            ) == Some(index)
        });

        assert!(all_round_trip);
    }

    #[test]
    fn palette_hit_test_ignores_section_gaps_and_separator() {
        let layout = palette_layout();
        let probes = [
            egui::pos2(
                layout.gates_width + PALETTE_SECTION_GAP / 2.0,
                PALETTE_SIZE / 2.0,
            ),
            egui::pos2(
                layout.separator_x + PALETTE_SEPARATOR_WIDTH / 2.0,
                PALETTE_SIZE / 2.0,
            ),
            egui::pos2(
                layout.separator_x + PALETTE_SEPARATOR_WIDTH + PALETTE_SECTION_GAP / 2.0,
                PALETTE_SIZE / 2.0,
            ),
        ];

        assert!(probes
            .iter()
            .all(|probe| palette_hit_test(*probe, &layout).is_none()));
    }
}
