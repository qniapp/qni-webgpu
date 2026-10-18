use eframe::egui;

use crate::constants::{
    PALETTE_DISPLAY_COLUMNS, PALETTE_DISPLAY_ROWS, PALETTE_GAP, PALETTE_MARGIN_X,
    PALETTE_PADDING_X, PALETTE_PADDING_Y, PALETTE_ROW_GAP, PALETTE_SECTION_GAP,
    PALETTE_SEPARATOR_WIDTH, PALETTE_SIZE,
};
use crate::gates::{
    default_palette_angle, palette_gate_kind, GateKind, ParametricAngle, PALETTE_DISPLAY_GATES,
    PALETTE_DISPLAY_INDICES, PALETTE_GATES_ROW1_INDICES, PALETTE_GATES_ROW2,
    PALETTE_GATES_ROW2_INDICES, PALETTE_GATE_COUNT, PALETTE_ROW1_COUNT,
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct PaletteLayout {
    pub(crate) total_width: f32,
    pub(crate) total_height: f32,
    /// Gates / Display divider, relative to the palette origin. Only the
    /// full palette has the Display section.
    pub(crate) separator: Option<egui::Rect>,
    grid: PaletteGrid,
}

#[derive(Clone, Copy, Debug)]
enum PaletteGrid {
    /// A restricted palette's entries per row.
    Rows {
        columns: usize,
    },
    Full(FullPaletteGrid),
}

/// The full palette: the first and second gate rows, each wrapped into
/// balanced rows of at most `row1_columns` / `row2_columns` entries, and the
/// Display section `display_columns` wide beside the top gate rows.
#[derive(Clone, Copy, Debug)]
struct FullPaletteGrid {
    row1_columns: usize,
    row2_columns: usize,
    display_columns: usize,
    display_x: f32,
}

/// A palette gate as it drops into the circuit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PaletteEntry {
    pub(crate) kind: GateKind,
    pub(crate) angle: Option<ParametricAngle>,
}

/// One palette cell: a gate, or the comment block that wraps circuit columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PaletteItem {
    Gate(PaletteEntry),
    CircuitBlock,
}

/// Restricted-palette token for the comment block.
pub(crate) const CIRCUIT_BLOCK_PALETTE_TOKEN: &str = "{}";

/// Flat index of the comment block in the full palette: after every gate,
/// in the last Display slot (bottom-right in the desktop 3 × 2 layout).
pub(crate) const PALETTE_CIRCUIT_BLOCK_INDEX: usize = PALETTE_GATE_COUNT;

/// Which items the palette offers. Indices are stable per palette: the full
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
    Restricted(Vec<PaletteItem>),
}

impl Palette {
    /// Parses circuit-JSON gate tokens such as `["|0>", "H", "P(π/4)"]`, plus
    /// `"{}"` for the comment block.
    pub(crate) fn restricted<S: AsRef<str>>(tokens: &[S]) -> Result<Self, String> {
        tokens
            .iter()
            .map(|token| {
                let token = token.as_ref();
                if token == CIRCUIT_BLOCK_PALETTE_TOKEN {
                    return Ok(PaletteItem::CircuitBlock);
                }
                crate::url_circuit::palette_token_to_gate(token)
                    .map(|(kind, angle)| {
                        PaletteItem::Gate(PaletteEntry {
                            kind,
                            angle: angle.or_else(|| default_palette_angle(kind)),
                        })
                    })
                    .ok_or_else(|| format!("unknown palette gate: {token}"))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Self::Restricted)
    }

    pub(crate) fn is_hidden(&self) -> bool {
        matches!(self, Self::Restricted(entries) if entries.is_empty())
    }

    pub(crate) fn item(&self, index: usize) -> Option<PaletteItem> {
        match self {
            Self::Full if index == PALETTE_CIRCUIT_BLOCK_INDEX => Some(PaletteItem::CircuitBlock),
            Self::Full => palette_gate_kind(index).map(|kind| {
                PaletteItem::Gate(PaletteEntry {
                    kind,
                    angle: default_palette_angle(kind),
                })
            }),
            Self::Restricted(entries) => entries.get(index).copied(),
        }
    }

    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Full => PALETTE_CIRCUIT_BLOCK_INDEX + 1,
            Self::Restricted(entries) => entries.len(),
        }
    }

    /// Whether the comment block can be placed, which also lets existing
    /// blocks be edited.
    pub(crate) fn offers_circuit_block(&self) -> bool {
        match self {
            Self::Full => true,
            Self::Restricted(entries) => entries.contains(&PaletteItem::CircuitBlock),
        }
    }

    /// Layout on a canvas `canvas_width` px wide.
    pub(crate) fn layout(&self, canvas_width: f32) -> PaletteLayout {
        match self {
            Self::Full => full_palette_layout(canvas_width),
            Self::Restricted(entries) => {
                let columns = restricted_columns(entries.len(), canvas_width);
                let rows = entries.len().div_ceil(columns.max(1));
                PaletteLayout {
                    total_width: palette_row_width(columns),
                    total_height: palette_column_height(rows),
                    separator: None,
                    grid: PaletteGrid::Rows { columns },
                }
            }
        }
    }

    /// Gate top-left relative to the palette panel top-left.
    pub(crate) fn local_pos(&self, index: usize, layout: &PaletteLayout) -> Option<egui::Pos2> {
        if index >= self.len() {
            return None;
        }
        match layout.grid {
            PaletteGrid::Rows { columns } => Some(cell_pos(index % columns, index / columns)),
            PaletteGrid::Full(grid) => grid.local_pos(index),
        }
    }

    /// Maps a position relative to the palette origin to the entry under it.
    /// Gaps, the separator and empty cells hit nothing.
    pub(crate) fn hit_test(&self, local_pos: egui::Pos2, layout: &PaletteLayout) -> Option<usize> {
        (0..self.len()).find(|&index| {
            self.local_pos(index, layout).is_some_and(|min| {
                egui::Rect::from_min_size(min, egui::vec2(PALETTE_SIZE, PALETTE_SIZE))
                    .contains(local_pos)
            })
        })
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
    /// two-row palette `LINE_Y` leaves room for (negative when a wrapped
    /// palette is taller). Circuit-space geometry (`LINE_Y`) stays fixed;
    /// screen conversion subtracts this offset, like the horizontal scroll.
    pub(crate) fn circuit_shift_y(&self, canvas_width: f32) -> f32 {
        palette_column_height(PALETTE_DISPLAY_ROWS) + 2.0 * PALETTE_PADDING_Y
            - self.panel_height(canvas_width)
    }
}

/// Entries per row of a restricted palette: everything in one row when the
/// panel fits between the 16px canvas margins, otherwise the fewest
/// rows that fit, filled evenly (9 entries on a 354 px canvas → 5 + 4).
fn restricted_columns(count: usize, canvas_width: f32) -> usize {
    if count == 0 {
        return 0;
    }
    let fit = (((palette_content_width(canvas_width) + PALETTE_GAP) / (PALETTE_SIZE + PALETTE_GAP))
        .floor() as usize)
        .max(1);
    balanced_columns(count, fit)
}

/// Widest palette content that keeps the panel between the canvas margins.
fn palette_content_width(canvas_width: f32) -> f32 {
    canvas_width - 2.0 * (PALETTE_MARGIN_X + PALETTE_PADDING_X)
}

/// Entries per row when `count` entries wrap into the fewest rows of at
/// most `limit`, filled evenly.
fn balanced_columns(count: usize, limit: usize) -> usize {
    count.div_ceil(count.div_ceil(limit))
}

fn cell_pos(col: usize, row: usize) -> egui::Pos2 {
    egui::pos2(
        col as f32 * (PALETTE_SIZE + PALETTE_GAP),
        row as f32 * (PALETTE_SIZE + PALETTE_ROW_GAP),
    )
}

fn palette_column_height(rows: usize) -> f32 {
    if rows == 0 {
        0.0
    } else {
        rows as f32 * PALETTE_SIZE + (rows - 1) as f32 * PALETTE_ROW_GAP
    }
}

fn palette_row_width(count: usize) -> f32 {
    if count == 0 {
        0.0
    } else {
        count as f32 * PALETTE_SIZE + (count - 1) as f32 * PALETTE_GAP
    }
}

/// Display section widths a full palette tries for each gate-row width:
/// the 3 × 2 block, one row beside the first gate row, then 2 × 3.
const DISPLAY_COLUMN_CHOICES: [usize; 3] =
    [PALETTE_DISPLAY_COLUMNS, PALETTE_DISPLAY_GATES.len() + 1, 2];

/// The first full-palette layout that fits `canvas_width`, trying gate rows
/// from one row each down to a single column and, for each, every
/// `DISPLAY_COLUMN_CHOICES` entry.
fn full_palette_layout(canvas_width: f32) -> PaletteLayout {
    let available = palette_content_width(canvas_width);
    let mut candidates = (1..=PALETTE_GATES_ROW2.len()).rev().flat_map(|limit| {
        DISPLAY_COLUMN_CHOICES
            .map(|display_columns| FullPaletteGrid::new(limit, display_columns).layout())
    });
    let narrowest = FullPaletteGrid::new(1, DISPLAY_COLUMN_CHOICES[2]).layout();
    candidates
        .find(|layout| layout.total_width <= available)
        .unwrap_or(narrowest)
}

impl FullPaletteGrid {
    fn new(limit: usize, display_columns: usize) -> Self {
        let mut grid = Self {
            row1_columns: balanced_columns(PALETTE_ROW1_COUNT, limit),
            row2_columns: balanced_columns(PALETTE_GATES_ROW2.len(), limit),
            display_columns,
            display_x: 0.0,
        };
        let beside_width = grid
            .gate_row_counts()
            .take(grid.display_rows())
            .map(palette_row_width)
            .fold(0.0, f32::max);
        grid.display_x = beside_width + 2.0 * PALETTE_SECTION_GAP + PALETTE_SEPARATOR_WIDTH;
        grid
    }

    fn row1_rows(&self) -> usize {
        PALETTE_ROW1_COUNT.div_ceil(self.row1_columns)
    }

    fn display_rows(&self) -> usize {
        (PALETTE_DISPLAY_GATES.len() + 1).div_ceil(self.display_columns)
    }

    /// Entry counts of the gate rows from top to bottom.
    fn gate_row_counts(&self) -> impl Iterator<Item = usize> {
        let row_counts = |count: usize, columns: usize| {
            (0..count.div_ceil(columns)).map(move |row| (count - row * columns).min(columns))
        };
        row_counts(PALETTE_ROW1_COUNT, self.row1_columns)
            .chain(row_counts(PALETTE_GATES_ROW2.len(), self.row2_columns))
    }

    fn layout(self) -> PaletteLayout {
        let display_rows = self.display_rows();
        let display_height = palette_column_height(display_rows);
        let gates_width = self
            .gate_row_counts()
            .map(palette_row_width)
            .fold(0.0, f32::max);
        PaletteLayout {
            total_width: gates_width.max(self.display_x + palette_row_width(self.display_columns)),
            total_height: palette_column_height(self.gate_row_counts().count().max(display_rows)),
            separator: Some(egui::Rect::from_min_size(
                egui::pos2(
                    self.display_x - PALETTE_SECTION_GAP - PALETTE_SEPARATOR_WIDTH,
                    0.0,
                ),
                egui::vec2(PALETTE_SEPARATOR_WIDTH, display_height),
            )),
            grid: PaletteGrid::Full(self),
        }
    }

    fn local_pos(&self, index: usize) -> Option<egui::Pos2> {
        if let Some(pos) = PALETTE_GATES_ROW1_INDICES
            .iter()
            .position(|candidate| *candidate == index)
        {
            return Some(cell_pos(pos % self.row1_columns, pos / self.row1_columns));
        }
        if let Some(pos) = PALETTE_GATES_ROW2_INDICES
            .iter()
            .position(|candidate| *candidate == index)
        {
            return Some(cell_pos(
                pos % self.row2_columns,
                self.row1_rows() + pos / self.row2_columns,
            ));
        }
        let slot = if index == PALETTE_CIRCUIT_BLOCK_INDEX {
            PALETTE_DISPLAY_GATES.len()
        } else {
            PALETTE_DISPLAY_INDICES
                .iter()
                .position(|candidate| *candidate == index)?
        };
        Some(
            cell_pos(slot % self.display_columns, slot / self.display_columns)
                + egui::vec2(self.display_x, 0.0),
        )
    }
}

pub(crate) fn palette_start_x(width: f32, layout: &PaletteLayout) -> f32 {
    // Odd-width viewports would otherwise place the palette on a half pixel,
    // making PNG glyph textures look thinner than the integer-aligned circuit.
    (width / 2.0 - layout.total_width / 2.0).round()
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
            .filter_map(|index| match palette.item(index) {
                Some(PaletteItem::Gate(entry)) => Some(entry.kind),
                _ => None,
            })
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

        let Some(PaletteItem::Gate(entry)) = palette.item(0) else {
            panic!("expected a phase gate");
        };
        assert_eq!(entry.angle, Some(ParametricAngle::default()));
    }

    #[test]
    fn restricted_phase_keeps_explicit_angle() {
        let palette = Palette::restricted(&["P(π/4)"]).unwrap();

        let Some(PaletteItem::Gate(entry)) = palette.item(0) else {
            panic!("expected a phase gate");
        };
        assert_eq!(entry.angle, ParametricAngle::parse_qni("π/4").ok());
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
        assert_eq!(Palette::Full.circuit_shift_y(DESKTOP), 0.0);
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
            Palette::Full.panel_height(DESKTOP)
        );
    }

    #[test]
    fn restricted_palette_has_no_display_separator() {
        let palette = Palette::restricted(&["H", "Bloch"]).unwrap();

        assert_eq!(palette.layout(WIDE).separator, None);
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

        assert_eq!(palette.layout(WIDE).total_height, PALETTE_SIZE);
    }

    #[test]
    fn narrow_canvas_wraps_restricted_palette_into_balanced_rows() {
        let palette = Palette::restricted(&PHASE_PALETTE).unwrap();

        assert_eq!(palette.layout(NARROW).total_width, palette_row_width(5));
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

    // Circuit areas of 1280 px, 1000 px, 844 px and 800 px canvases: the
    // canvas minus the central panel's 8 px margins.
    const DESKTOP: f32 = 1264.0;
    const LAPTOP: f32 = 984.0;
    const PHONE_LANDSCAPE: f32 = 828.0;
    const SMALL: f32 = 784.0;
    const DENSITY_DISPLAY: usize = 25;

    fn sweep() -> impl Iterator<Item = f32> {
        (320..=1600).step_by(40).map(|width| width as f32)
    }

    fn cell(palette: &Palette, index: usize, layout: &PaletteLayout) -> egui::Rect {
        egui::Rect::from_min_size(
            palette.local_pos(index, layout).unwrap(),
            egui::vec2(PALETTE_SIZE, PALETTE_SIZE),
        )
    }

    #[test]
    fn desktop_full_palette_keeps_its_two_row_width() {
        assert_eq!(Palette::Full.layout(DESKTOP).total_width, 1010.0);
    }

    #[test]
    fn desktop_arithmetic_gates_follow_rz() {
        let layout = Palette::Full.layout(DESKTOP);
        let positions = [12, 36, 37].map(|index| Palette::Full.local_pos(index, &layout));

        assert_eq!(
            positions,
            [12.0, 13.0, 14.0].map(|column| Some(cell_pos(column as usize, 0)))
        );
    }

    #[test]
    fn small_window_arithmetic_gates_wrap_after_rz() {
        let layout = Palette::Full.layout(SMALL);

        assert_eq!(
            [12, 36, 37].map(|index| Palette::Full.local_pos(index, &layout)),
            [
                Some(cell_pos(4, 1)),
                Some(cell_pos(5, 1)),
                Some(cell_pos(6, 1))
            ]
        );
    }

    #[test]
    fn desktop_density_display_sits_in_second_row() {
        let layout = Palette::Full.layout(DESKTOP);

        assert_eq!(
            Palette::Full.local_pos(DENSITY_DISPLAY, &layout).unwrap().y,
            PALETTE_SIZE + PALETTE_ROW_GAP
        );
    }

    #[test]
    fn laptop_keeps_display_section_in_two_rows() {
        let layout = Palette::Full.layout(LAPTOP);

        assert_eq!(
            Palette::Full.local_pos(DENSITY_DISPLAY, &layout).unwrap().y,
            PALETTE_SIZE + PALETTE_ROW_GAP
        );
    }

    #[test]
    fn laptop_separator_spans_two_rows() {
        assert_eq!(
            Palette::Full.layout(LAPTOP).separator.unwrap().height(),
            2.0 * PALETTE_SIZE + PALETTE_ROW_GAP
        );
    }

    #[test]
    fn laptop_pushes_circuit_down_one_row() {
        assert_eq!(
            Palette::Full.circuit_shift_y(LAPTOP),
            -(PALETTE_SIZE + PALETTE_ROW_GAP)
        );
    }

    #[test]
    fn phone_landscape_wraps_into_four_rows() {
        assert_eq!(
            Palette::Full.circuit_shift_y(PHONE_LANDSCAPE),
            -2.0 * (PALETTE_SIZE + PALETTE_ROW_GAP)
        );
    }

    #[test]
    fn phone_landscape_stacks_display_section_three_wide() {
        let layout = Palette::Full.layout(PHONE_LANDSCAPE);

        assert_eq!(
            Palette::Full.local_pos(DENSITY_DISPLAY, &layout).unwrap().y,
            PALETTE_SIZE + PALETTE_ROW_GAP
        );
    }

    #[test]
    fn small_window_wraps_gates_into_four_rows() {
        assert_eq!(
            Palette::Full.circuit_shift_y(SMALL),
            -2.0 * (PALETTE_SIZE + PALETTE_ROW_GAP)
        );
    }

    #[test]
    fn small_window_wraps_first_gate_row_evenly() {
        let layout = Palette::Full.layout(SMALL);

        assert_eq!(
            Palette::Full.local_pos(8, &layout),
            Some(egui::pos2(0.0, PALETTE_SIZE + PALETTE_ROW_GAP))
        );
    }

    #[test]
    fn full_palette_panel_stays_inside_canvas_margins() {
        let clipped = sweep()
            .filter(|&width| {
                let layout = Palette::Full.layout(width);
                let left = palette_start_x(width, &layout) - PALETTE_PADDING_X;
                let right = left + layout.total_width + 2.0 * PALETTE_PADDING_X;
                left < PALETTE_MARGIN_X || right > width - PALETTE_MARGIN_X
            })
            .collect::<Vec<_>>();

        assert_eq!(clipped, Vec::<f32>::new());
    }

    #[test]
    fn full_palette_entries_stay_inside_panel() {
        let outside = sweep()
            .flat_map(|width| (0..Palette::Full.len()).map(move |index| (width, index)))
            .filter(|&(width, index)| {
                let layout = Palette::Full.layout(width);
                let bounds = egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(layout.total_width, layout.total_height),
                );
                !bounds.contains_rect(cell(&Palette::Full, index, &layout))
            })
            .collect::<Vec<_>>();

        assert_eq!(outside, Vec::<(f32, usize)>::new());
    }

    #[test]
    fn full_palette_entries_clear_the_separator() {
        let overlapping = sweep()
            .flat_map(|width| (0..Palette::Full.len()).map(move |index| (width, index)))
            .filter(|&(width, index)| {
                let layout = Palette::Full.layout(width);
                layout
                    .separator
                    .unwrap()
                    .intersects(cell(&Palette::Full, index, &layout))
            })
            .collect::<Vec<_>>();

        assert_eq!(overlapping, Vec::<(f32, usize)>::new());
    }

    #[test]
    fn full_palette_hit_test_round_trips_every_entry() {
        let misses = sweep()
            .flat_map(|width| (0..Palette::Full.len()).map(move |index| (width, index)))
            .filter(|&(width, index)| {
                let layout = Palette::Full.layout(width);
                Palette::Full.hit_test(cell(&Palette::Full, index, &layout).center(), &layout)
                    != Some(index)
            })
            .collect::<Vec<_>>();

        assert_eq!(misses, Vec::<(f32, usize)>::new());
    }

    #[test]
    fn full_palette_hit_test_ignores_separator() {
        let hits = [DESKTOP, LAPTOP, SMALL]
            .map(|width| {
                let layout = Palette::Full.layout(width);
                Palette::Full.hit_test(layout.separator.unwrap().center(), &layout)
            })
            .to_vec();

        assert_eq!(hits, vec![None; 3]);
    }

    #[test]
    fn full_palette_hit_test_ignores_gap_after_first_gate() {
        let layout = Palette::Full.layout(DESKTOP);

        assert_eq!(
            Palette::Full.hit_test(
                egui::pos2(PALETTE_SIZE + PALETTE_GAP / 2.0, PALETTE_SIZE / 2.0),
                &layout
            ),
            None
        );
    }
    #[test]
    fn full_palette_offers_the_circuit_block_last() {
        assert_eq!(
            Palette::Full.item(Palette::Full.len() - 1),
            Some(PaletteItem::CircuitBlock)
        );
    }

    #[test]
    fn desktop_circuit_block_sits_in_the_display_sections_bottom_right_slot() {
        let layout = Palette::Full.layout(DESKTOP);

        assert_eq!(
            Palette::Full.local_pos(PALETTE_CIRCUIT_BLOCK_INDEX, &layout),
            Some(egui::pos2(
                layout.separator.unwrap().max.x
                    + PALETTE_SECTION_GAP
                    + 2.0 * (PALETTE_SIZE + PALETTE_GAP),
                PALETTE_SIZE + PALETTE_ROW_GAP,
            ))
        );
    }

    #[test]
    fn palette_hit_test_finds_the_circuit_block_slot() {
        let layout = Palette::Full.layout(DESKTOP);
        let local = Palette::Full
            .local_pos(PALETTE_CIRCUIT_BLOCK_INDEX, &layout)
            .unwrap()
            + egui::vec2(20.0, 20.0);

        assert_eq!(
            Palette::Full.hit_test(local, &layout),
            Some(PALETTE_CIRCUIT_BLOCK_INDEX)
        );
    }

    #[test]
    fn full_palette_has_no_item_after_the_circuit_block() {
        let layout = Palette::Full.layout(DESKTOP);

        assert_eq!(
            Palette::Full.local_pos(PALETTE_CIRCUIT_BLOCK_INDEX + 1, &layout),
            None
        );
    }

    #[test]
    fn restricted_palette_accepts_the_circuit_block_token() {
        let palette = Palette::restricted(&["H", "{}"]).unwrap();

        assert_eq!(palette.item(1), Some(PaletteItem::CircuitBlock));
    }

    #[test]
    fn circuit_block_is_a_distinct_palette_item() {
        assert_eq!(
            Palette::Full.item(PALETTE_CIRCUIT_BLOCK_INDEX),
            Some(PaletteItem::CircuitBlock)
        );
    }

    #[test]
    fn restricted_palette_circuit_block_is_a_distinct_item() {
        assert_eq!(
            Palette::restricted(&["{}"]).unwrap().item(0),
            Some(PaletteItem::CircuitBlock)
        );
    }

    #[test]
    fn full_palette_has_no_item_after_its_last_slot() {
        assert_eq!(Palette::Full.item(Palette::Full.len()), None);
    }

    #[test]
    fn restricted_block_hit_test_round_trips_after_wrapping() {
        let palette = Palette::restricted(&["H", "X", "Y", "Z", "{}"]).unwrap();
        let layout = palette.layout(200.0);
        let local = palette.local_pos(4, &layout).unwrap();

        assert_eq!(
            palette.hit_test(
                local + egui::vec2(PALETTE_SIZE / 2.0, PALETTE_SIZE / 2.0),
                &layout
            ),
            Some(4)
        );
    }

    #[test]
    fn restricted_block_wraps_onto_another_row() {
        let palette = Palette::restricted(&["H", "X", "Y", "Z", "{}"]).unwrap();
        let layout = palette.layout(200.0);
        let local = palette.local_pos(4, &layout).unwrap();

        assert!(local.y > 0.0);
    }

    #[test]
    fn restricted_palette_rejects_an_open_block_marker() {
        assert!(Palette::restricted(&["{"]).is_err());
    }

    #[test]
    fn restricted_palette_with_the_token_offers_the_circuit_block() {
        assert!(Palette::restricted(&["H", "{}"])
            .unwrap()
            .offers_circuit_block());
    }

    #[test]
    fn restricted_palette_without_the_token_lacks_the_circuit_block() {
        assert!(!Palette::restricted(&["H"]).unwrap().offers_circuit_block());
    }

    #[test]
    fn full_palette_offers_the_circuit_block() {
        assert!(Palette::Full.offers_circuit_block());
    }
}
