use eframe::egui;

use crate::constants::{
    CAROUSEL_ARROW_INSET, CAROUSEL_ARROW_SIZE, CAROUSEL_DOT_GAP, CAROUSEL_DOT_SIZE,
    CAROUSEL_DOT_TOP_GAP, CAROUSEL_GATES_PER_PAGE, CAROUSEL_GATE_GAP, CAROUSEL_GATE_SIZE,
    CAROUSEL_PADDING_BOTTOM, CAROUSEL_PADDING_TOP, VERTICAL_SCREEN_INSET,
};
use crate::gates::{
    PALETTE_DISPLAY_INDICES, PALETTE_GATES_ROW1_INDICES, PALETTE_GATES_ROW2_INDICES,
};

use super::palette::PALETTE_CIRCUIT_BLOCK_INDEX;
use super::Palette;

/// What a press on the palette carousel lands on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CarouselHit {
    /// Palette entry index of the gate under the pointer.
    Gate(usize),
    Prev,
    Next,
    Dot(usize),
}

/// The vertical layout's palette: a card fixed to the bottom of the screen
/// showing `CAROUSEL_GATES_PER_PAGE` gates per page, with arrows and page
/// dots. Pages stop at both ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PaletteCarousel {
    rect: egui::Rect,
    page_count: usize,
}

impl PaletteCarousel {
    /// Carousel for `entry_count` palette entries at the bottom of
    /// `screen_rect`.
    pub(crate) fn new(screen_rect: egui::Rect, entry_count: usize) -> Self {
        let size = egui::vec2(
            screen_rect.width() - 2.0 * VERTICAL_SCREEN_INSET,
            Self::height(),
        );
        let min = egui::pos2(
            screen_rect.min.x + VERTICAL_SCREEN_INSET,
            screen_rect.max.y - VERTICAL_SCREEN_INSET - size.y,
        );
        Self {
            rect: egui::Rect::from_min_size(min, size),
            page_count: entry_count.div_ceil(CAROUSEL_GATES_PER_PAGE),
        }
    }

    pub(crate) fn height() -> f32 {
        CAROUSEL_PADDING_TOP
            + CAROUSEL_GATE_SIZE
            + CAROUSEL_DOT_TOP_GAP
            + CAROUSEL_DOT_SIZE
            + CAROUSEL_PADDING_BOTTOM
    }

    pub(crate) fn rect(&self) -> egui::Rect {
        self.rect
    }

    pub(crate) fn page_count(&self) -> usize {
        self.page_count
    }

    /// Arrows and dots only appear when there is more than one page.
    pub(crate) fn paged(&self) -> bool {
        self.page_count > 1
    }

    pub(crate) fn clamp_page(&self, page: usize) -> usize {
        page.min(self.page_count.saturating_sub(1))
    }

    pub(crate) fn prev_page(&self, page: usize) -> usize {
        self.clamp_page(page.saturating_sub(1))
    }

    pub(crate) fn next_page(&self, page: usize) -> usize {
        self.clamp_page(page + 1)
    }

    /// Screen rect of slot `slot` (`0..CAROUSEL_GATES_PER_PAGE`) on the
    /// shown page. The row is centred in the card.
    pub(crate) fn slot_rect(&self, slot: usize) -> egui::Rect {
        let count = CAROUSEL_GATES_PER_PAGE as f32;
        let row_width = count * CAROUSEL_GATE_SIZE + (count - 1.0) * CAROUSEL_GATE_GAP;
        let left = (self.rect.center().x - row_width / 2.0).round();
        egui::Rect::from_min_size(
            egui::pos2(
                left + slot as f32 * (CAROUSEL_GATE_SIZE + CAROUSEL_GATE_GAP),
                self.rect.min.y + CAROUSEL_PADDING_TOP,
            ),
            egui::vec2(CAROUSEL_GATE_SIZE, CAROUSEL_GATE_SIZE),
        )
    }

    pub(crate) fn prev_rect(&self) -> egui::Rect {
        egui::Rect::from_min_size(
            egui::pos2(
                self.rect.min.x + CAROUSEL_ARROW_INSET,
                self.slot_rect(0).min.y,
            ),
            egui::vec2(CAROUSEL_ARROW_SIZE, CAROUSEL_ARROW_SIZE),
        )
    }

    pub(crate) fn next_rect(&self) -> egui::Rect {
        egui::Rect::from_min_size(
            egui::pos2(
                self.rect.max.x - CAROUSEL_ARROW_INSET - CAROUSEL_ARROW_SIZE,
                self.slot_rect(0).min.y,
            ),
            egui::vec2(CAROUSEL_ARROW_SIZE, CAROUSEL_ARROW_SIZE),
        )
    }

    pub(crate) fn dot_center(&self, page: usize) -> egui::Pos2 {
        let count = self.page_count as f32;
        let width = count * CAROUSEL_DOT_SIZE + (count - 1.0).max(0.0) * CAROUSEL_DOT_GAP;
        let left = (self.rect.center().x - width / 2.0).round();
        egui::pos2(
            left + page as f32 * (CAROUSEL_DOT_SIZE + CAROUSEL_DOT_GAP) + CAROUSEL_DOT_SIZE / 2.0,
            self.slot_rect(0).max.y + CAROUSEL_DOT_TOP_GAP + CAROUSEL_DOT_SIZE / 2.0,
        )
    }

    /// Tap area of a page dot: the dot plus half the gap on each side and
    /// the whole bottom band of the card.
    fn dot_rect(&self, page: usize) -> egui::Rect {
        let center = self.dot_center(page);
        egui::Rect::from_center_size(
            center,
            egui::vec2(
                CAROUSEL_DOT_SIZE + CAROUSEL_DOT_GAP,
                CAROUSEL_DOT_SIZE + CAROUSEL_DOT_TOP_GAP + CAROUSEL_PADDING_BOTTOM,
            ),
        )
    }

    /// Hit test a screen position on `page`.
    pub(crate) fn hit_test(
        &self,
        palette: &Palette,
        page: usize,
        pos: egui::Pos2,
    ) -> Option<CarouselHit> {
        if !self.rect.contains(pos) {
            return None;
        }
        let page = self.clamp_page(page);
        if let Some(index) = (0..CAROUSEL_GATES_PER_PAGE)
            .find(|&slot| self.slot_rect(slot).contains(pos))
            .and_then(|slot| palette.carousel_entry(page * CAROUSEL_GATES_PER_PAGE + slot))
        {
            return Some(CarouselHit::Gate(index));
        }
        if !self.paged() {
            return None;
        }
        if self.prev_rect().contains(pos) {
            return Some(CarouselHit::Prev);
        }
        if self.next_rect().contains(pos) {
            return Some(CarouselHit::Next);
        }
        (0..self.page_count)
            .find(|&dot| self.dot_rect(dot).contains(pos))
            .map(CarouselHit::Dot)
    }

    /// Page shown after pressing `hit` on `page`.
    pub(crate) fn page_after(&self, page: usize, hit: CarouselHit) -> usize {
        match hit {
            CarouselHit::Prev => self.prev_page(page),
            CarouselHit::Next => self.next_page(page),
            CarouselHit::Dot(dot) => self.clamp_page(dot),
            CarouselHit::Gate(_) => self.clamp_page(page),
        }
    }
}

impl Palette {
    /// Palette entry index at carousel position `position`. The full
    /// palette follows its desktop reading order: the first gate row, the
    /// second gate row, then the display blocks and comment block.
    pub(crate) fn carousel_entry(&self, position: usize) -> Option<usize> {
        match self {
            Self::Full => PALETTE_GATES_ROW1_INDICES
                .into_iter()
                .chain(PALETTE_GATES_ROW2_INDICES)
                .chain(PALETTE_DISPLAY_INDICES)
                .chain(std::iter::once(PALETTE_CIRCUIT_BLOCK_INDEX))
                .nth(position),
            Self::Restricted(entries) => (position < entries.len()).then_some(position),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gates::GateKind;
    use crate::layout::PaletteItem;

    fn carousel() -> PaletteCarousel {
        PaletteCarousel::new(
            egui::Rect::from_min_size(egui::pos2(8.0, 8.0), egui::vec2(374.0, 828.0)),
            Palette::Full.len(),
        )
    }

    fn page_kinds(page: usize) -> Vec<GateKind> {
        (0..CAROUSEL_GATES_PER_PAGE)
            .filter_map(|slot| Palette::Full.carousel_entry(page * CAROUSEL_GATES_PER_PAGE + slot))
            .filter_map(|index| match Palette::Full.item(index) {
                Some(PaletteItem::Gate(entry)) => Some(entry.kind),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn full_palette_has_ten_pages() {
        assert_eq!(carousel().page_count(), 10);
    }

    #[test]
    fn restricted_three_entries_has_one_page() {
        let palette = Palette::restricted(&["H", "•", "X"]).unwrap();
        let carousel = PaletteCarousel::new(
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(390.0, 844.0)),
            palette.len(),
        );

        assert_eq!(carousel.page_count(), 1);
    }

    #[test]
    fn first_page_is_h_x_y_z() {
        assert_eq!(
            page_kinds(0),
            vec![GateKind::H, GateKind::X, GateKind::Y, GateKind::Z]
        );
    }

    #[test]
    fn second_page_is_sqrt_x_s_s_dagger_t() {
        assert_eq!(
            page_kinds(1),
            vec![GateKind::SqrtX, GateKind::S, GateKind::SDagger, GateKind::T]
        );
    }

    #[test]
    fn fourth_page_is_rz_increment_decrement_swap() {
        assert_eq!(
            page_kinds(3),
            vec![
                GateKind::Rz,
                GateKind::Increment,
                GateKind::Decrement,
                GateKind::Swap
            ]
        );
    }

    #[test]
    fn eighth_page_is_spacer_and_postselections() {
        assert_eq!(
            page_kinds(7),
            vec![
                GateKind::Spacer,
                GateKind::PostSelectOff,
                GateKind::PostSelectOn,
                GateKind::PostSelectAntiX
            ]
        );
    }

    #[test]
    fn ninth_page_ends_postselections_and_starts_displays() {
        assert_eq!(
            page_kinds(8),
            vec![
                GateKind::PostSelectX,
                GateKind::BlochDisplay,
                GateKind::ProbabilityDisplay,
                GateKind::SampleDisplay
            ]
        );
    }

    #[test]
    fn tenth_page_ends_with_amplitude_density_and_comment() {
        assert_eq!(
            (0..CAROUSEL_GATES_PER_PAGE)
                .map(|slot| Palette::Full.carousel_entry(9 * CAROUSEL_GATES_PER_PAGE + slot))
                .collect::<Vec<_>>(),
            vec![Some(24), Some(25), Some(PALETTE_CIRCUIT_BLOCK_INDEX), None]
        );
    }

    #[test]
    fn carousel_order_visits_every_entry_once() {
        let mut indices = (0..Palette::Full.len())
            .filter_map(|position| Palette::Full.carousel_entry(position))
            .collect::<Vec<_>>();
        indices.sort_unstable();

        assert_eq!(indices, (0..Palette::Full.len()).collect::<Vec<_>>());
    }

    #[test]
    fn card_sits_sixteen_px_above_the_canvas_bottom() {
        assert_eq!(carousel().rect().max.y, 844.0 - 16.0);
    }

    #[test]
    fn gate_row_is_centred_in_the_card() {
        let carousel = carousel();
        let row = carousel.slot_rect(0).union(carousel.slot_rect(3));

        assert_eq!(row.center().x, carousel.rect().center().x);
    }

    #[test]
    fn page1_slot0_hits_sqrt_x() {
        let carousel = carousel();
        let hit = carousel.hit_test(&Palette::Full, 1, carousel.slot_rect(0).center());

        assert_eq!(
            hit.and_then(|hit| match hit {
                CarouselHit::Gate(index) => match Palette::Full.item(index) {
                    Some(PaletteItem::Gate(entry)) => Some(entry.kind),
                    _ => None,
                },
                _ => None,
            }),
            Some(GateKind::SqrtX)
        );
    }

    #[test]
    fn page3_slot1_hits_increment() {
        let carousel = carousel();

        assert_eq!(
            carousel.hit_test(&Palette::Full, 3, carousel.slot_rect(1).center()),
            Some(CarouselHit::Gate(36))
        );
    }

    #[test]
    fn page3_slot2_hits_decrement() {
        let carousel = carousel();

        assert_eq!(
            carousel.hit_test(&Palette::Full, 3, carousel.slot_rect(2).center()),
            Some(CarouselHit::Gate(37))
        );
    }

    #[test]
    fn prev_arrow_hit() {
        let carousel = carousel();

        assert_eq!(
            carousel.hit_test(&Palette::Full, 3, carousel.prev_rect().center()),
            Some(CarouselHit::Prev)
        );
    }

    #[test]
    fn next_arrow_hit() {
        let carousel = carousel();

        assert_eq!(
            carousel.hit_test(&Palette::Full, 3, carousel.next_rect().center()),
            Some(CarouselHit::Next)
        );
    }

    #[test]
    fn dot5_hit() {
        let carousel = carousel();

        assert_eq!(
            carousel.hit_test(&Palette::Full, 0, carousel.dot_center(5)),
            Some(CarouselHit::Dot(5))
        );
    }

    #[test]
    fn gap_between_gates_hits_nothing() {
        let carousel = carousel();
        let gap = egui::pos2(
            carousel.slot_rect(0).max.x + CAROUSEL_GATE_GAP / 2.0,
            carousel.slot_rect(0).center().y,
        );

        assert_eq!(carousel.hit_test(&Palette::Full, 0, gap), None);
    }

    #[test]
    fn single_page_hides_arrows_from_hit_test() {
        let palette = Palette::restricted(&["H", "•", "X"]).unwrap();
        let carousel = PaletteCarousel::new(
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(390.0, 844.0)),
            palette.len(),
        );

        assert_eq!(
            carousel.hit_test(&palette, 0, carousel.next_rect().center()),
            None
        );
    }

    #[test]
    fn empty_slot_on_last_restricted_page_hits_nothing() {
        let palette = Palette::restricted(&["H", "X", "Y", "Z", "S"]).unwrap();
        let carousel = PaletteCarousel::new(
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(390.0, 844.0)),
            palette.len(),
        );

        assert_eq!(
            carousel.hit_test(&palette, 1, carousel.slot_rect(1).center()),
            None
        );
    }

    #[test]
    fn last_page_hits_the_comment_block() {
        let carousel = carousel();

        assert_eq!(
            carousel.hit_test(&Palette::Full, 9, carousel.slot_rect(2).center()),
            Some(CarouselHit::Gate(PALETTE_CIRCUIT_BLOCK_INDEX))
        );
    }

    #[test]
    fn next_on_last_page_stays() {
        assert_eq!(carousel().page_after(9, CarouselHit::Next), 9);
    }

    #[test]
    fn prev_on_first_page_stays() {
        assert_eq!(carousel().page_after(0, CarouselHit::Prev), 0);
    }

    #[test]
    fn next_advances_one_page() {
        assert_eq!(carousel().page_after(2, CarouselHit::Next), 3);
    }

    #[test]
    fn dot_jumps_to_its_page() {
        assert_eq!(carousel().page_after(0, CarouselHit::Dot(6)), 6);
    }
}
