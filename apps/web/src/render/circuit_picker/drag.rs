use eframe::egui;

use crate::app::circuit_library::{persist_library, CircuitEntry};
use crate::app::QniApp;
use crate::colors::Colors;

use super::chrome::{paint_dragged_row_background, paint_picker_item_text};
use super::constants::{
    DRAG_ACTIVATE_DISTANCE_SQ, EDGE_AUTO_SCROLL_MAX_PX, EDGE_AUTO_SCROLL_ZONE, ITEM_HEIGHT,
    LIVE_REORDER_ANIM_SECONDS,
};

impl QniApp {
    pub(super) fn animated_picker_item_rect(
        &self,
        ui: &egui::Ui,
        entry: &CircuitEntry,
        index: usize,
        rect: egui::Rect,
        drag_active: bool,
    ) -> egui::Rect {
        if !drag_active {
            return rect;
        }
        let target_rect = rect.translate(egui::vec2(0.0, self.picker.visual_row_shift(index)));
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
                .animate_value_with_time(id, target_rect.top(), LIVE_REORDER_ANIM_SECONDS);
        target_rect.translate(egui::vec2(0.0, animated_y - target_rect.top()))
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

    pub(super) fn update_picker_drag(
        &mut self,
        ctx: &egui::Context,
        row_rects: &[(usize, egui::Rect)],
    ) {
        if !self.picker.drag_in_progress() {
            return;
        }
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        if let Some(pointer_pos) = picker_pointer_pos(ctx) {
            self.picker
                .promote_pending_drag(pointer_pos, DRAG_ACTIVATE_DISTANCE_SQ);
            self.update_picker_drag_slot(pointer_pos, row_rects);
        }
        if ctx.input(|input| input.pointer.any_released()) {
            self.finish_picker_live_drag();
        }
    }

    pub(super) fn update_picker_drag_auto_scroll(
        &mut self,
        ctx: &egui::Context,
        pane_rect: egui::Rect,
        content_height: f32,
        current_offset: f32,
        min_scroll_offset: f32,
        max_scroll_offset: Option<f32>,
    ) {
        if self.picker.dragged_row().is_none() {
            self.picker.clear_pending_scroll_offset();
            return;
        }
        let Some(pointer_pos) = picker_pointer_pos(ctx) else {
            self.picker.clear_pending_scroll_offset();
            return;
        };
        let max_offset = (content_height - pane_rect.height()).max(0.0);
        if max_offset <= 0.0 {
            self.picker.clear_pending_scroll_offset();
            return;
        }
        let speed = edge_auto_scroll_speed(pointer_pos.y, pane_rect);
        if speed == 0.0 {
            self.picker.clear_pending_scroll_offset();
            return;
        }
        let clamped_min_scroll_offset = min_scroll_offset.min(max_offset);
        let clamped_max_scroll_offset = max_scroll_offset
            .map(|boundary| (boundary - pane_rect.height()).max(0.0))
            .unwrap_or(max_offset)
            .min(max_offset);
        if speed < 0.0 && current_offset <= clamped_min_scroll_offset {
            self.picker.clear_pending_scroll_offset();
            return;
        }
        if speed > 0.0 && current_offset >= clamped_max_scroll_offset {
            self.picker.clear_pending_scroll_offset();
            return;
        }
        let min_offset = if speed < 0.0 {
            clamped_min_scroll_offset
        } else {
            0.0
        };
        let max_offset = if speed > 0.0 {
            clamped_max_scroll_offset
        } else {
            max_offset
        };
        let next_offset = (current_offset + speed).clamp(min_offset, max_offset);
        if (next_offset - current_offset).abs() <= f32::EPSILON {
            self.picker.clear_pending_scroll_offset();
            return;
        }
        self.picker.set_pending_scroll_offset(next_offset);
        ctx.request_repaint();
    }

    fn update_picker_drag_slot(
        &mut self,
        pointer_pos: egui::Pos2,
        row_rects: &[(usize, egui::Rect)],
    ) {
        if self.picker.dragged_row().is_none() {
            return;
        }
        let Some(slot) = target_slot_from_pointer(pointer_pos.y, row_rects) else {
            return;
        };
        if self.picker.set_active_drag_slot(slot) {
            self.picker.set_focused_index(slot);
        }
    }

    fn finish_picker_live_drag(&mut self) {
        if let Some((source, slot)) = self.picker.finish_drag() {
            self.library.move_to_slot(source, slot);
            self.library.bump_updated_at();
            persist_library(&self.library);
        }
    }

    pub(super) fn paint_picker_dragged_row(
        &self,
        ui: &egui::Ui,
        colors: &Colors,
        entries: &[CircuitEntry],
        pane_rect: egui::Rect,
        section_divider_y: Option<f32>,
    ) {
        let Some((index, source_row_left, row_width, pointer_offset_y, pinned_top)) =
            self.picker.drag_paint_row()
        else {
            return;
        };
        let Some(pointer_pos) = picker_pointer_pos(ui.ctx()) else {
            return;
        };
        let Some(entry) = entries.get(index) else {
            return;
        };
        let mut min_top = pane_rect.top();
        let mut max_top = (pane_rect.bottom() - ITEM_HEIGHT).max(pane_rect.top());
        if let Some(divider_y) = section_divider_y {
            if entry.is_sample() {
                max_top = max_top.min(divider_y - ITEM_HEIGHT);
            } else if entry.is_user() {
                min_top = min_top.max(divider_y);
            }
        }
        let max_top = max_top.max(pane_rect.top());
        let min_top = min_top.min(max_top);
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

fn picker_pointer_pos(ctx: &egui::Context) -> Option<egui::Pos2> {
    ctx.input(|input| {
        input
            .pointer
            .hover_pos()
            .or_else(|| input.pointer.interact_pos())
            .or_else(|| input.pointer.latest_pos())
    })
}

fn target_slot_from_pointer(pointer_y: f32, row_rects: &[(usize, egui::Rect)]) -> Option<usize> {
    let mut slot = 0;
    for (index, (_, rect)) in row_rects.iter().enumerate() {
        if pointer_y < rect.center().y {
            break;
        }
        slot = index;
    }
    row_rects.get(slot).map(|(entry_index, _)| *entry_index)
}

fn edge_auto_scroll_speed(pointer_y: f32, pane_rect: egui::Rect) -> f32 {
    if pointer_y < pane_rect.top() + EDGE_AUTO_SCROLL_ZONE {
        let depth =
            (pane_rect.top() + EDGE_AUTO_SCROLL_ZONE - pointer_y).clamp(0.0, EDGE_AUTO_SCROLL_ZONE);
        -(depth / EDGE_AUTO_SCROLL_ZONE) * EDGE_AUTO_SCROLL_MAX_PX
    } else if pointer_y > pane_rect.bottom() - EDGE_AUTO_SCROLL_ZONE {
        let depth = (pointer_y - (pane_rect.bottom() - EDGE_AUTO_SCROLL_ZONE))
            .clamp(0.0, EDGE_AUTO_SCROLL_ZONE);
        (depth / EDGE_AUTO_SCROLL_ZONE) * EDGE_AUTO_SCROLL_MAX_PX
    } else {
        0.0
    }
}
