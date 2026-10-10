use eframe::egui;

use crate::constants::{
    PALETTE_DISPLAY_COLUMNS, PALETTE_DISPLAY_ROWS, PALETTE_GAP, PALETTE_MARGIN_X,
    PALETTE_PADDING_X, PALETTE_PADDING_Y, PALETTE_ROW_GAP, PALETTE_SECTION_GAP,
    PALETTE_SEPARATOR_WIDTH, PALETTE_SIZE,
};
use crate::gates::{
    default_palette_angle, palette_gate_kind, GateKind, ParametricAngle, PALETTE_DISPLAY_GATES,
    PALETTE_DISPLAY_INDICES, PALETTE_GATES_ROW2, PALETTE_GATES_ROW2_INDICES, PALETTE_GATE_COUNT,
    PALETTE_ROW1_COUNT,
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct PaletteLayout {
    pub(crate) total_width: f32,
    pub(crate) total_height: f32,
    pub(crate) gates_width: f32,
    /// Gates / Display divider. Only the full palette has the Display section.
    pub(crate) separator_x: Option<f32>,
    pub(crate) display_x: f32,
    pub(crate) display_width: f32,
    /// Entries per row of a restricted palette. The full palette keeps its
    /// fixed grid and ignores this.
    pub(crate) columns: usize,
}

/// A palette gate as it drops into the circuit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PaletteEntry {
    pub(crate) kind: GateKind,
    pub(crate) angle: Option<ParametricAngle>,
}

/// Which gates the palette offers. Indices are stable per palette: the full
/// palette keeps its historical flat indices, a restricted palette numbers
/// its entries left to right.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum Palette {
    #[default]
    Full,
    /// qni tutorial embeds list only the gates a lesson needs (the arguments
    /// of qni's `mini_qni` Liquid filter). The entries sit in one row and
    /// wrap into balanced rows when the canvas is too narrow. Empty hides
    /// the palette panel.
    Restricted(Vec<PaletteEntry>),
}

impl Palette {
    /// Parses circuit-JSON gate tokens such as `["|0>", "H", "P(π/4)"]`.
    pub(crate) fn restricted<S: AsRef<str>>(tokens: &[S]) -> Result<Self, String> {
        tokens
            .iter()
            .map(|token| {
                let token = token.as_ref();
                crate::url_circuit::palette_token_to_gate(token)
                    .map(|(kind, angle)| PaletteEntry {
                        kind,
                        angle: angle.or_else(|| default_palette_angle(kind)),
                    })
                    .ok_or_else(|| format!("unknown palette gate: {token}"))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Self::Restricted)
    }

    pub(crate) fn is_hidden(&self) -> bool {
        matches!(self, Self::Restricted(entries) if entries.is_empty())
    }

    pub(crate) fn entry(&self, index: usize) -> Option<PaletteEntry> {
        match self {
            Self::Full => palette_gate_kind(index).map(|kind| PaletteEntry {
                kind,
                angle: default_palette_angle(kind),
            }),
            Self::Restricted(entries) => entries.get(index).copied(),
        }
    }

    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Full => PALETTE_GATE_COUNT,
            Self::Restricted(entries) => entries.len(),
        }
    }

    /// Layout on a canvas `canvas_width` px wide. Only a restricted palette
    /// depends on the width; the full palette keeps its fixed grid.
    pub(crate) fn layout(&self, canvas_width: f32) -> PaletteLayout {
        match self {
            Self::Full => palette_layout(),
            Self::Restricted(entries) => {
                let columns = restricted_columns(entries.len(), canvas_width);
                let rows = entries.len().div_ceil(columns.max(1));
                let width = palette_row_width(columns);
                PaletteLayout {
                    total_width: width,
                    total_height: palette_column_height(rows),
                    gates_width: width,
                    separator_x: None,
                    display_x: width,
                    display_width: 0.0,
                    columns,
                }
            }
        }
    }

    /// Gate top-left relative to the palette panel top-left.
    pub(crate) fn local_pos(&self, index: usize, layout: &PaletteLayout) -> Option<egui::Pos2> {
        match self {
            Self::Full => palette_gate_local_pos(index, layout),
            Self::Restricted(entries) => (index < entries.len()).then(|| {
                egui::pos2(
                    (index % layout.columns) as f32 * (PALETTE_SIZE + PALETTE_GAP),
                    (index / layout.columns) as f32 * (PALETTE_SIZE + PALETTE_ROW_GAP),
                )
            }),
        }
    }

    pub(crate) fn hit_test(&self, local_pos: egui::Pos2, layout: &PaletteLayout) -> Option<usize> {
        match self {
            Self::Full => palette_hit_test(local_pos, layout),
            Self::Restricted(entries) => {
                let row = restricted_row_from_y(local_pos.y)?;
                let col = col_from_x(local_pos.x).filter(|col| *col < layout.columns)?;
                Some(row * layout.columns + col).filter(|index| *index < entries.len())
            }
        }
    }

    /// Height of the palette panel including its padding; `0` when hidden.
    pub(crate) fn panel_height(&self, canvas_width: f32) -> f32 {
        if self.is_hidden() {
            0.0
        } else {
            self.layout(canvas_width).total_height + 2.0 * PALETTE_PADDING_Y
        }
    }

    /// How far the circuit moves up because this palette is shorter than the
    /// full two-row palette (negative when a wrapped palette is taller).
    /// Circuit-space geometry (`LINE_Y`) stays fixed; screen conversion
    /// subtracts this offset, like the horizontal scroll.
    pub(crate) fn circuit_shift_y(&self, canvas_width: f32) -> f32 {
        Self::Full.panel_height(canvas_width) - self.panel_height(canvas_width)
    }
}

/// Entries per row of a restricted palette: everything in one row when the
/// panel fits between the spacing-4 canvas margins, otherwise the fewest
/// rows that fit, filled evenly (9 entries on a 354 px canvas → 5 + 4).
fn restricted_columns(count: usize, canvas_width: f32) -> usize {
    if count == 0 {
        return 0;
    }
    let available = canvas_width - 2.0 * (PALETTE_MARGIN_X + PALETTE_PADDING_X);
    let fit = (((available + PALETTE_GAP) / (PALETTE_SIZE + PALETTE_GAP)).floor() as usize).max(1);
    count.div_ceil(count.div_ceil(fit))
}

fn palette_column_height(rows: usize) -> f32 {
    if rows == 0 {
        0.0
    } else {
        rows as f32 * PALETTE_SIZE + (rows - 1) as f32 * PALETTE_ROW_GAP
    }
}

fn restricted_row_from_y(y: f32) -> Option<usize> {
    if y < 0.0 {
        return None;
    }
    let row = (y / (PALETTE_SIZE + PALETTE_ROW_GAP)).floor() as usize;
    (y - row as f32 * (PALETTE_SIZE + PALETTE_ROW_GAP) <= PALETTE_SIZE).then_some(row)
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

fn palette_layout() -> PaletteLayout {
    let gates_row1_width = palette_row_width(PALETTE_ROW1_COUNT);
    let gates_row2_width = palette_row_width(PALETTE_GATES_ROW2.len());
    let gates_width = gates_row1_width.max(gates_row2_width);
    let separator_x = gates_width + PALETTE_SECTION_GAP;
    let display_x = separator_x + PALETTE_SEPARATOR_WIDTH + PALETTE_SECTION_GAP;
    let display_width = palette_grid_width(PALETTE_DISPLAY_COLUMNS);
    PaletteLayout {
        total_width: display_x + display_width,
        total_height: palette_column_height(PALETTE_DISPLAY_ROWS),
        gates_width,
        separator_x: Some(separator_x),
        display_x,
        display_width,
        columns: PALETTE_ROW1_COUNT,
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
/// right.
fn palette_gate_local_pos(index: usize, layout: &PaletteLayout) -> Option<egui::Pos2> {
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
fn palette_hit_test(local_pos: egui::Pos2, layout: &PaletteLayout) -> Option<usize> {
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

    // Circuit-area widths of the qni tutorial embeds on 1440 px and 390 px
    // pages: the 1068 px / 354 px canvases minus the central panel's 8 px
    // margins.
    const WIDE: f32 = 1052.0;
    const NARROW: f32 = 338.0;
    const PHASE_PALETTE: [&str; 9] = [
        "H", "X", "Y", "Z", "P(π/2)", "X^½", "Rx(π/2)", "Ry(π/2)", "Rz(π/2)",
    ];

    fn kinds(palette: &Palette) -> Vec<GateKind> {
        (0..palette.len())
            .filter_map(|index| palette.entry(index).map(|entry| entry.kind))
            .collect()
    }

    #[test]
    fn restricted_palette_accepts_every_mini_qni_token() {
        let palette =
            Palette::restricted(&["|0>", "|1>", "H", "X", "Y", "Z", "P", "•", "Bloch"]).unwrap();

        assert_eq!(
            kinds(&palette),
            vec![
                GateKind::Write0,
                GateKind::Write1,
                GateKind::H,
                GateKind::X,
                GateKind::Y,
                GateKind::Z,
                GateKind::Phase,
                GateKind::Control,
                GateKind::BlochDisplay,
            ]
        );
    }

    #[test]
    fn restricted_bare_phase_drops_with_mini_qni_default_angle() {
        let palette = Palette::restricted(&["P"]).unwrap();

        assert_eq!(
            palette.entry(0).unwrap().angle,
            Some(ParametricAngle::default())
        );
    }

    #[test]
    fn restricted_phase_keeps_explicit_angle() {
        let palette = Palette::restricted(&["P(π/4)"]).unwrap();

        assert_eq!(
            palette.entry(0).unwrap().angle,
            ParametricAngle::parse_qni("π/4").ok()
        );
    }

    #[test]
    fn restricted_palette_rejects_unknown_token() {
        assert!(Palette::restricted(&["H", "Foo"]).is_err());
    }

    #[test]
    fn restricted_palette_rejects_span_suffix() {
        assert!(Palette::restricted(&["QFT3"]).is_err());
    }

    #[test]
    fn empty_restricted_palette_is_hidden() {
        assert!(Palette::restricted::<&str>(&[]).unwrap().is_hidden());
    }

    #[test]
    fn full_palette_keeps_circuit_in_place() {
        assert_eq!(Palette::Full.circuit_shift_y(WIDE), 0.0);
    }

    #[test]
    fn single_row_palette_lifts_circuit_by_one_row() {
        let palette = Palette::restricted(&["H", "X"]).unwrap();

        assert_eq!(
            palette.circuit_shift_y(WIDE),
            PALETTE_SIZE + PALETTE_ROW_GAP
        );
    }

    #[test]
    fn hidden_palette_lifts_circuit_by_whole_panel() {
        let palette = Palette::restricted::<&str>(&[]).unwrap();

        assert_eq!(
            palette.circuit_shift_y(WIDE),
            Palette::Full.panel_height(WIDE)
        );
    }

    #[test]
    fn restricted_palette_has_no_display_separator() {
        let palette = Palette::restricted(&["H", "Bloch"]).unwrap();

        assert_eq!(palette.layout(WIDE).separator_x, None);
    }

    #[test]
    fn restricted_hit_test_round_trips_entries() {
        let palette = Palette::restricted(&["|0>", "H", "•"]).unwrap();
        let layout = palette.layout(WIDE);
        let hits = (0..palette.len())
            .map(|index| {
                let local = palette.local_pos(index, &layout).unwrap();
                palette.hit_test(
                    local + egui::vec2(PALETTE_SIZE / 2.0, PALETTE_SIZE / 2.0),
                    &layout,
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(hits, vec![Some(0), Some(1), Some(2)]);
    }

    #[test]
    fn wide_canvas_keeps_restricted_palette_in_one_row() {
        let palette = Palette::restricted(&PHASE_PALETTE).unwrap();

        assert_eq!(palette.layout(WIDE).columns, PHASE_PALETTE.len());
    }

    #[test]
    fn narrow_canvas_wraps_restricted_palette_into_balanced_rows() {
        let palette = Palette::restricted(&PHASE_PALETTE).unwrap();

        assert_eq!(palette.layout(NARROW).columns, 5);
    }

    #[test]
    fn wrapped_palette_panel_fits_inside_canvas_margins() {
        let palette = Palette::restricted(&PHASE_PALETTE).unwrap();
        let layout = palette.layout(NARROW);
        let left = palette_start_x(NARROW, &layout) - PALETTE_PADDING_X;
        let right = left + layout.total_width + 2.0 * PALETTE_PADDING_X;

        assert!(left >= PALETTE_MARGIN_X && right <= NARROW - PALETTE_MARGIN_X);
    }

    #[test]
    fn wrapped_palette_starts_second_row_below_first() {
        let palette = Palette::restricted(&PHASE_PALETTE).unwrap();
        let layout = palette.layout(NARROW);

        assert_eq!(
            palette.local_pos(5, &layout),
            Some(egui::pos2(0.0, PALETTE_SIZE + PALETTE_ROW_GAP))
        );
    }

    #[test]
    fn two_row_wrapped_palette_keeps_circuit_in_place() {
        let palette = Palette::restricted(&PHASE_PALETTE).unwrap();

        assert_eq!(palette.circuit_shift_y(NARROW), 0.0);
    }

    #[test]
    fn three_row_wrapped_palette_pushes_circuit_down_one_row() {
        let palette = Palette::restricted(&["H"; 7]).unwrap();

        // 7 entries at most 3 per row (a 200 px canvas) → 3 + 3 + 1.
        assert_eq!(
            palette.circuit_shift_y(200.0),
            -(PALETTE_SIZE + PALETTE_ROW_GAP)
        );
    }

    #[test]
    fn wrapped_hit_test_round_trips_entries() {
        let palette = Palette::restricted(&PHASE_PALETTE).unwrap();
        let layout = palette.layout(NARROW);
        let round_trips = (0..palette.len()).all(|index| {
            let local = palette.local_pos(index, &layout).unwrap();
            palette.hit_test(
                local + egui::vec2(PALETTE_SIZE / 2.0, PALETTE_SIZE / 2.0),
                &layout,
            ) == Some(index)
        });

        assert!(round_trips);
    }

    #[test]
    fn wrapped_hit_test_ignores_empty_cell_after_last_entry() {
        let palette = Palette::restricted(&PHASE_PALETTE).unwrap();
        let layout = palette.layout(NARROW);
        let empty = egui::pos2(
            4.0 * (PALETTE_SIZE + PALETTE_GAP) + PALETTE_SIZE / 2.0,
            PALETTE_SIZE + PALETTE_ROW_GAP + PALETTE_SIZE / 2.0,
        );

        assert_eq!(palette.hit_test(empty, &layout), None);
    }

    #[test]
    fn restricted_hit_test_ignores_second_row() {
        let palette = Palette::restricted(&["H"]).unwrap();
        let layout = palette.layout(WIDE);

        assert_eq!(
            palette.hit_test(
                egui::pos2(20.0, PALETTE_SIZE + PALETTE_ROW_GAP + 20.0),
                &layout
            ),
            None
        );
    }

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
    fn palette_hit_test_finds_density_display_slot() {
        let layout = palette_layout();
        let local = palette_gate_local_pos(25, &layout).unwrap() + egui::vec2(20.0, 20.0);

        assert_eq!(palette_hit_test(local, &layout), Some(25));
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
                layout.separator_x.unwrap() + PALETTE_SEPARATOR_WIDTH / 2.0,
                PALETTE_SIZE / 2.0,
            ),
            egui::pos2(
                layout.separator_x.unwrap() + PALETTE_SEPARATOR_WIDTH + PALETTE_SECTION_GAP / 2.0,
                PALETTE_SIZE / 2.0,
            ),
        ];

        assert!(probes
            .iter()
            .all(|probe| palette_hit_test(*probe, &layout).is_none()));
    }
}
