//! Gate drag controller facade.
//!
//! The submodules keep qni's SNAP / DROP / RESIZE style event boundaries
//! explicit while `gate_input` stays a thin egui adapter.

mod circuit_block;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod circuit_block_tests;
mod drop;
mod hover;
pub(super) mod pan;
mod preview;
mod resize;
mod scroll;
mod start;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod touch_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod vertical_tests;

use eframe::egui;

use super::{CircuitColumnIndex, QniApp};
use crate::constants::{GATE_SIZE, PALETTE_ROW_Y, SLOT_SPACING, STEP_OUTSET_Y};
use crate::layout::{
    layout_metrics, nearest_slot_index, palette_start_x, CarouselHit, CircuitOrientation,
    CircuitProjection, LayoutMetrics, Palette, PaletteCarousel, PaletteLayout,
};

#[derive(Clone, Copy, Debug)]
pub(super) struct DragPointer {
    pub(super) screen_pos: Option<egui::Pos2>,
    pub(super) local_pos: Option<egui::Pos2>,
    pub(super) down: bool,
    pub(super) start: bool,
    pub(super) shift_at_start: Option<bool>,
    pub(super) released: bool,
    /// `false` while the vertical layout's pointer is over a panel: columns
    /// hidden there must not take a snap or a drop.
    pub(super) snap_allowed: bool,
}

#[derive(Clone, Debug)]
enum PaletteInput {
    Grid {
        origin: egui::Pos2,
        rect: egui::Rect,
        layout: PaletteLayout,
    },
    Carousel(PaletteCarousel),
}

#[derive(Clone, Debug)]
pub(super) struct CircuitInputGeometry {
    pub(super) metrics: LayoutMetrics,
    orientation: CircuitOrientation,
    /// Screen rect of the vertical layout's circuit area. Gates and steps
    /// outside it are hidden under the panels, so they take no hits.
    pub(super) circuit_clip: Option<egui::Rect>,
    palette: PaletteInput,
}

impl CircuitInputGeometry {
    pub(super) fn new(
        content_rect: egui::Rect,
        screen_rect: egui::Rect,
        layout_qubits: usize,
        min_slots: usize,
        palette: &Palette,
    ) -> Self {
        let palette_layout = palette.layout(screen_rect.width());
        let palette_start_x = palette_start_x(screen_rect.width(), &palette_layout);
        let palette_origin = egui::pos2(
            screen_rect.min.x + palette_start_x,
            screen_rect.min.y + PALETTE_ROW_Y,
        );
        let palette_rect = egui::Rect::from_min_size(
            palette_origin,
            egui::vec2(palette_layout.total_width, palette_layout.total_height),
        );
        let metrics = layout_metrics(content_rect.width(), layout_qubits, min_slots);
        Self {
            metrics,
            orientation: CircuitOrientation::Horizontal,
            circuit_clip: None,
            palette: PaletteInput::Grid {
                origin: palette_origin,
                rect: palette_rect,
                layout: palette_layout,
            },
        }
    }

    pub(super) fn for_app(app: &QniApp, content_rect: egui::Rect, screen_rect: egui::Rect) -> Self {
        match app.orientation {
            CircuitOrientation::Horizontal => Self::new(
                content_rect,
                screen_rect,
                app.layout_qubits(),
                app.min_circuit_slots(),
                &app.palette,
            ),
            CircuitOrientation::Vertical => Self {
                metrics: app.circuit_layout_metrics(content_rect),
                orientation: CircuitOrientation::Vertical,
                circuit_clip: Some(content_rect),
                palette: PaletteInput::Carousel(PaletteCarousel::new(
                    screen_rect,
                    app.palette.len(),
                )),
            },
        }
    }

    /// Circuit space rotated like the screen, at the circuit-space origin.
    /// Display blocks lay out their cells upright in this frame.
    pub(super) fn upright(&self) -> CircuitProjection {
        CircuitProjection::new(self.orientation, egui::Pos2::ZERO)
    }

    pub(super) fn is_vertical(&self) -> bool {
        self.orientation.is_vertical()
    }

    /// Whether a pointer at `screen_pos` may hit gates and steps.
    pub(super) fn circuit_hit_allowed(&self, screen_pos: Option<egui::Pos2>) -> bool {
        match (self.circuit_clip, screen_pos) {
            (Some(clip), Some(pos)) => clip.contains(pos),
            (Some(_), None) => false,
            (None, _) => true,
        }
    }

    /// Carousel target under `screen_pos` in the vertical layout.
    pub(super) fn carousel_hit(
        &self,
        app: &QniApp,
        screen_pos: egui::Pos2,
    ) -> Option<(PaletteCarousel, CarouselHit)> {
        let PaletteInput::Carousel(carousel) = self.palette else {
            return None;
        };
        if app.library.active_locked() {
            return None;
        }
        carousel
            .hit_test(&app.palette, app.palette_page, screen_pos)
            .map(|hit| (carousel, hit))
    }

    /// Palette entry a press at `screen_pos` picks up.
    fn pressed_palette_entry(&self, app: &QniApp, screen_pos: egui::Pos2) -> Option<usize> {
        match &self.palette {
            PaletteInput::Grid { origin, layout, .. } => {
                let local = egui::pos2(screen_pos.x - origin.x, screen_pos.y - origin.y);
                app.palette.hit_test(local, layout)
            }
            PaletteInput::Carousel(_) => match self.carousel_hit(app, screen_pos)?.1 {
                CarouselHit::Gate(index) => Some(index),
                _ => None,
            },
        }
    }

    /// Palette entry the pointer at `screen_pos` hovers.
    fn hovered_palette_entry(&self, app: &QniApp, screen_pos: egui::Pos2) -> Option<usize> {
        match &self.palette {
            PaletteInput::Grid { rect, .. } if !rect.contains(screen_pos) => None,
            _ => self.pressed_palette_entry(app, screen_pos),
        }
    }
}

pub(super) struct DragController;

/// Column index the pointer is hovering over for step-preview.
/// Returns `None` when outside the slot row / range.
fn step_at_cursor(cursor: egui::Pos2, metrics: &LayoutMetrics) -> Option<CircuitColumnIndex> {
    if metrics.slot_centers.is_empty() || metrics.line_ys.is_empty() {
        return None;
    }
    let top = metrics.line_ys[0] - STEP_OUTSET_Y;
    let bottom = metrics.line_ys[metrics.line_ys.len() - 1] + STEP_OUTSET_Y;
    if cursor.y < top || cursor.y > bottom {
        return None;
    }
    // The initial step occupies one full slot width up to the first gate's
    // left edge. The column-zero step begins at that edge, not half a slot
    // before its center. Coordinates are circuit-local even in narrow gutters.
    let first_gate_left = metrics.slot_left - GATE_SIZE / 2.0;
    let initial_left = first_gate_left - SLOT_SPACING;
    if cursor.x >= initial_left && cursor.x < first_gate_left {
        return Some(CircuitColumnIndex::INITIAL);
    }
    if cursor.x < first_gate_left || cursor.x > metrics.slot_right + SLOT_SPACING * 0.5 {
        return None;
    }
    let (slot, dist) = nearest_slot_index(cursor.x, &metrics.slot_centers)?;
    if dist <= SLOT_SPACING * 0.5 {
        Some(CircuitColumnIndex::new(slot))
    } else {
        None
    }
}

fn reset_drag_frame_state(app: &mut QniApp) {
    app.drag_repaint_deadline = None;
    app.drag_repaint_pending = false;
}
