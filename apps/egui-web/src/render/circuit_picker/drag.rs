use eframe::egui;

use crate::app::circuit_library::{persist_library, CircuitEntry};
use crate::app::QniApp;
use crate::colors::Colors;

use super::chrome::{drag_clamp_bounds, paint_dragged_row_background, paint_picker_item_text};
use super::constants::{DRAG_ACTIVATE_DISTANCE_SQ, ITEM_HEIGHT, LIVE_REORDER_ANIM_SECONDS};

impl QniApp {
    pub(super) fn animated_picker_item_rect(
        &self,
        ui: &egui::Ui,
        entry: &CircuitEntry,
        rect: egui::Rect,
        drag_active: bool,
    ) -> egui::Rect {
        if !drag_active {
            return rect;
        }
        if self.picker.active_drag_index().is_some_and(|index| {
            self.library
                .entries
                .get(index)
                .is_some_and(|active| active.id == entry.id)
        }) {
            return rect;
        }
        // Scope FLIP memory to the current drag. Otherwise egui's value animation
        // can reuse an unfinished row-y interpolation from the previous drag and
        // make unrelated rows twitch on the next mousedown.
        let id = ui.make_persistent_id((
            "circuit-picker-row-y",
            self.picker_drag_animation_epoch,
            &entry.id,
        ));
        let animated_y =
            ui.ctx()
                .animate_value_with_time(id, rect.top(), LIVE_REORDER_ANIM_SECONDS);
        rect.translate(egui::vec2(0.0, animated_y - rect.top()))
    }

    pub(super) fn update_picker_drag_suppression(&mut self, ctx: &egui::Context) {
        let (primary_down, primary_released) = ctx.input(|input| {
            (
                input.pointer.primary_down(),
                input.pointer.primary_released(),
            )
        });
        if self.picker_drag_suppressed_until_release && !primary_down {
            self.picker_drag_suppressed_until_release = false;
        }
        if self.picker_submenu_toggle_suppressed_until_release && !primary_down && !primary_released
        {
            self.picker_submenu_toggle_suppressed_until_release = false;
        }
    }

    pub(super) fn update_picker_drag(&mut self, ctx: &egui::Context, row_rects: &[egui::Rect]) {
        if !self.picker.drag_in_progress() {
            return;
        }
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        let pointer_pos = ctx.input(|input| {
            input
                .pointer
                .hover_pos()
                .or_else(|| input.pointer.interact_pos())
        });
        if let Some(pointer_pos) = pointer_pos {
            self.picker.promote_pending_drag(
                pointer_pos,
                DRAG_ACTIVATE_DISTANCE_SQ,
                &self.library.entries,
            );
            self.live_swap_picker_drag(pointer_pos, row_rects);
        }
        if ctx.input(|input| input.pointer.any_released()) {
            self.finish_picker_live_drag();
        }
    }

    fn live_swap_picker_drag(&mut self, pointer_pos: egui::Pos2, row_rects: &[egui::Rect]) {
        let Some((_, _, _, pointer_offset_y)) = self.picker.dragged_row() else {
            return;
        };
        let Some((min_top, max_top)) = drag_clamp_bounds(row_rects) else {
            return;
        };
        loop {
            let Some(index) = self.picker.active_drag_index() else {
                return;
            };
            let Some(current_rect) = row_rects.get(index) else {
                return;
            };
            let wanted_top = (pointer_pos.y - pointer_offset_y).clamp(min_top, max_top);
            let visual_center = wanted_top + current_rect.height() / 2.0;
            if index > 0
                && row_rects
                    .get(index - 1)
                    .is_some_and(|above| visual_center <= above.center().y)
            {
                let next_index = index - 1;
                self.library.swap_adjacent(next_index, index);
                self.picker.set_active_drag_index(next_index);
                self.picker.set_focused_index(next_index);
                self.picker.mark_drag_reordered();
                continue;
            }
            if index + 1 < self.library.entries.len()
                && row_rects
                    .get(index + 1)
                    .is_some_and(|below| visual_center >= below.center().y)
            {
                let next_index = index + 1;
                self.library.swap_adjacent(index, next_index);
                self.picker.set_active_drag_index(next_index);
                self.picker.set_focused_index(next_index);
                self.picker.mark_drag_reordered();
                continue;
            }
            break;
        }
    }

    fn finish_picker_live_drag(&mut self) {
        if self.picker.finish_drag() {
            self.library.bump_updated_at();
            persist_library(&self.library);
        }
    }

    pub(super) fn paint_picker_dragged_row(
        &self,
        ui: &egui::Ui,
        colors: &Colors,
        entries: &[CircuitEntry],
        row_rects: &[egui::Rect],
    ) {
        let Some((index, source_row_left, row_width, pointer_offset_y, pinned_top)) =
            self.picker.drag_paint_row()
        else {
            return;
        };
        let Some(pointer_pos) = ui.ctx().input(|input| {
            input
                .pointer
                .hover_pos()
                .or_else(|| input.pointer.interact_pos())
        }) else {
            return;
        };
        let Some(entry) = entries.get(index) else {
            return;
        };
        let Some((min_top, max_top)) = drag_clamp_bounds(row_rects) else {
            return;
        };
        let wanted_top = pinned_top
            .unwrap_or(pointer_pos.y - pointer_offset_y)
            .clamp(min_top, max_top);
        let rect = egui::Rect::from_min_size(
            egui::pos2(source_row_left, wanted_top),
            egui::vec2(row_width, ITEM_HEIGHT),
        );
        paint_dragged_row_background(ui.painter(), colors, rect);
        let text_stop =
            egui::Rect::from_min_size(egui::pos2(rect.right(), rect.center().y), egui::Vec2::ZERO);
        paint_picker_item_text(ui, colors, rect, text_stop, entry, true, 255);
    }
}
