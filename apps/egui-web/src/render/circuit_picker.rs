use eframe::egui;

use crate::app::circuit_library::CircuitEntry;
use crate::app::circuit_picker_state::PickerState;
use crate::app::QniApp;
use crate::colors::{with_alpha, Colors};

mod action;
mod chrome;
mod constants;
mod drag;
mod rename;

use action::PickerAction;
use chrome::{
    apply_items_scrollbar_style, footer, paint_chevron, paint_divider, paint_kebab,
    paint_picker_item_text, paint_resize_separator, popover_frame,
    publish_picker_dropdown_geometry_json, publish_picker_resize_geometry_json,
    publish_picker_submenu_geometry_json, submenu_item,
};
use constants::*;
impl QniApp {
    pub(crate) fn show_circuit_picker(
        &mut self,
        ui: &mut egui::Ui,
        colors: &Colors,
        ctx: &egui::Context,
    ) {
        self.update_picker_drag_suppression(ctx);
        self.handle_picker_keyboard(ctx);
        let trigger = self.show_picker_trigger(ui, colors);
        if trigger.clicked() {
            if self.picker.is_open() {
                self.picker.close();
            } else {
                self.picker = PickerState::open(self.library.active_index());
            }
            ctx.request_repaint();
        }
        if trigger.has_focus()
            && ui.input(|input| {
                input.key_pressed(egui::Key::Enter) || input.key_pressed(egui::Key::Space)
            })
        {
            self.picker = PickerState::open_with_focus(self.library.active_index());
            ctx.request_repaint();
        }
        if let Some(dropdown_rect) = self.show_picker_dropdown(ctx, colors, trigger.rect) {
            self.picker_overlay_rect = Some(dropdown_rect);
            self.handle_picker_outside_click(ctx, trigger.rect, dropdown_rect);
        } else {
            self.picker_overlay_rect = None;
        }
    }

    fn show_picker_trigger(&self, ui: &mut egui::Ui, colors: &Colors) -> egui::Response {
        let font = egui::FontId::new(14.0, egui::FontFamily::Proportional); // text-sm = 14px.
        let name = self.active_circuit_name();
        let galley = egui::WidgetText::from(
            egui::RichText::new(name.to_owned())
                .font(font.clone())
                .color(colors.text_strong),
        )
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            TRIGGER_NAME_MAX_WIDTH,
            font,
        );
        let trigger_w = TRIGGER_PAD_LEFT
            + galley.size().x
            + TRIGGER_NAME_CHEVRON_GAP
            + TRIGGER_CHEVRON_W
            + TRIGGER_PAD_RIGHT;
        let trigger_size = egui::vec2(trigger_w, TRIGGER_HEIGHT);
        let (rect, mut response) = ui.allocate_exact_size(trigger_size, egui::Sense::click());
        let hovered = response.hovered() || self.picker.is_open();
        let hover_t = ui
            .ctx()
            .animate_bool_with_time(response.id.with("hover"), hovered, 0.12);
        if hover_t > 0.0 {
            ui.painter().rect_filled(
                rect,
                egui::CornerRadius::same(ITEM_RADIUS),
                with_alpha(colors.toolbar_hover_bg, (255.0 * hover_t) as u8), // Flexoki ui.
            );
        }
        ui.painter().galley(
            egui::pos2(
                rect.left() + TRIGGER_PAD_LEFT,
                rect.center().y - galley.size().y / 2.0,
            ),
            galley,
            colors.text_strong,
        );
        let open_t = ui.ctx().animate_bool_with_time(
            response.id.with("chevron"),
            self.picker.is_open(),
            0.16,
        );
        let chev_center = egui::pos2(
            rect.right() - TRIGGER_PAD_RIGHT - TRIGGER_CHEVRON_W / 2.0,
            rect.center().y,
        );
        paint_chevron(ui.painter(), chev_center, open_t, colors.toolbar_icon);
        if name.chars().count() > 40 {
            response = response.on_hover_text(name.to_owned());
        }
        response
    }

    fn show_picker_dropdown(
        &mut self,
        ctx: &egui::Context,
        colors: &Colors,
        trigger_rect: egui::Rect,
    ) -> Option<egui::Rect> {
        if !self.picker.is_open() {
            return None;
        }
        // Attach the picker to the topbar bottom with no gap below the bar.
        // The trigger sits inside the toolbar's py-1.5 (6px) vertical padding.
        let pos = trigger_rect.left_bottom() + egui::vec2(0.0, TOPBAR_BOTTOM_OFFSET);
        let entries = self.library.entries.clone();
        let active_id = self.library.active_id.clone();
        let focused_index = if self.picker.focus_visible() {
            self.picker.focused_index()
        } else {
            None
        };
        let submenu_index = self.picker.submenu_index();
        let renaming_id = self.picker.renaming_id().map(str::to_owned);
        let max_items_height = picker_max_items_height(entries.len(), ctx.content_rect().height());
        self.picker.clamp_items_height(max_items_height);
        let mut actions = Vec::new();
        let mut row_rects = Vec::with_capacity(entries.len());
        let mut kebab_rects = Vec::with_capacity(entries.len());
        let area = egui::Area::new(egui::Id::new("circuit-picker-dropdown"))
            .order(egui::Order::Tooltip)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                popover_frame(colors).show(ui, |ui| {
                    ui.set_min_width(DROPDOWN_WIDTH - 12.0);
                    ui.set_max_width(DROPDOWN_WIDTH - 12.0);
                    let items_height = self.picker.items_height();
                    let previous_style = ui.style().clone();
                    let previous_context_style = ctx.style();
                    apply_items_scrollbar_style(ui, colors);
                    ctx.style_mut(|style| {
                        style.animation_time = ITEMS_SCROLLBAR_FADE_SECONDS;
                    });
                    let scroll_output = egui::ScrollArea::vertical()
                        .id_salt("circuit-picker-items")
                        .max_height(items_height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_style(previous_style.clone());
                            ui.spacing_mut().item_spacing.y = 0.0; // space-y-0: content cap equals row stack height.
                            ui.add_space(ITEMS_CONTENT_PADDING_Y); // pt-1.5 = 6px.
                            for (index, entry) in entries.iter().enumerate() {
                                let rects = self.show_picker_item(
                                    ui,
                                    colors,
                                    entry,
                                    index,
                                    entry.id == active_id,
                                    focused_index == Some(index),
                                    submenu_index == Some(index),
                                    renaming_id.as_deref() == Some(entry.id.as_str()),
                                    &entries,
                                    &mut actions,
                                );
                                row_rects.push(rects.row);
                                kebab_rects.push(rects.kebab);
                            }
                            self.update_picker_drag(ctx, &row_rects);
                            self.paint_picker_dragged_row(
                                ui,
                                colors,
                                &self.library.entries,
                                &row_rects,
                            );
                            ui.add_space(ITEMS_CONTENT_PADDING_Y); // pb-1.5 = 6px.
                        });
                    ui.set_style(previous_style);
                    ctx.set_style(previous_context_style);
                    ui.add_space(RESIZE_HANDLE_MARGIN_TOP_Y); // mt-0.
                    let (handle_rect, handle_response) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), RESIZE_HANDLE_H),
                        egui::Sense::click_and_drag(),
                    );
                    if !self.picker.resize_drag_active() {
                        if handle_response.is_pointer_button_down_on() {
                            if let Some(pointer) = handle_response.interact_pointer_pos() {
                                self.picker.start_resize_drag(pointer.y);
                            }
                        } else if handle_response.drag_started() {
                            if let Some(pointer) = handle_response.interact_pointer_pos() {
                                self.picker
                                    .start_resize_drag(pointer.y - handle_response.drag_delta().y);
                            }
                        }
                    }
                    if self.picker.resize_drag_active() {
                        if let Some(pointer) = ctx.input(|input| input.pointer.latest_pos()) {
                            self.picker.update_resize_drag(pointer.y, max_items_height);
                        }
                        ctx.request_repaint();
                    }
                    if handle_response.drag_stopped()
                        || (self.picker.resize_drag_active()
                            && !ctx.input(|input| input.pointer.primary_down()))
                    {
                        self.picker.finish_resize_drag();
                    }
                    let handle_active =
                        handle_response.hovered() || self.picker.resize_drag_active();
                    if handle_active {
                        ui.output_mut(|output| output.cursor_icon = egui::CursorIcon::ResizeRow);
                    }
                    paint_resize_separator(ui, colors, handle_rect, handle_active);
                    ui.add_space(RESIZE_HANDLE_MARGIN_BOTTOM_Y); // mb-0.5 = 2px.
                    let (footer_response, footer_rect) = footer(ui, colors);
                    if footer_response.clicked() {
                        actions.push(PickerAction::Create);
                    }
                    publish_picker_resize_geometry_json(
                        self.picker.items_height(),
                        max_items_height,
                        scroll_output.inner_rect,
                        handle_rect,
                        footer_rect,
                        row_rects.first().copied(),
                        row_rects.last().copied(),
                        scroll_output.state.offset.y,
                        handle_response.hovered(),
                        self.picker.resize_drag_active(),
                    );
                });
            });
        let dropdown_rect = area.response.rect;
        publish_picker_dropdown_geometry_json(trigger_rect, dropdown_rect, TOPBAR_BOTTOM_OFFSET);
        let mut deferred_actions = Vec::with_capacity(actions.len());
        for action in actions {
            match action {
                PickerAction::OpenSubmenu(index) => {
                    self.apply_picker_action(PickerAction::OpenSubmenu(index), ctx);
                }
                PickerAction::CloseSubmenuFromTrigger { suppress_click } => {
                    self.apply_picker_action(
                        PickerAction::CloseSubmenuFromTrigger { suppress_click },
                        ctx,
                    );
                }
                action => deferred_actions.push(action),
            }
        }
        let submenu_rect = self
            .picker
            .submenu_index()
            .and_then(|index| {
                kebab_rects
                    .get(index)
                    .copied()
                    .zip(row_rects.get(index).copied())
                    .map(|(anchor, parent_row)| (index, anchor, parent_row))
            })
            .and_then(|(index, anchor, parent_row)| {
                self.show_picker_submenu(
                    ctx,
                    colors,
                    index,
                    anchor,
                    parent_row,
                    &mut deferred_actions,
                )
            });
        let picker_rect = submenu_rect
            .map(|rect| dropdown_rect.union(rect))
            .unwrap_or(dropdown_rect);
        for action in deferred_actions {
            self.apply_picker_action(action, ctx);
        }
        self.handle_submenu_outside_click(ctx, dropdown_rect, submenu_rect);
        Some(picker_rect)
    }

    #[allow(clippy::too_many_arguments)]
    fn show_picker_item(
        &mut self,
        ui: &mut egui::Ui,
        colors: &Colors,
        entry: &CircuitEntry,
        index: usize,
        active: bool,
        focused: bool,
        submenu_open: bool,
        renaming: bool,
        entries: &[CircuitEntry],
        actions: &mut Vec<PickerAction>,
    ) -> PickerItemRects {
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), ITEM_HEIGHT),
            egui::Sense::click_and_drag(),
        );
        let kebab_rect = egui::Rect::from_center_size(
            egui::pos2(
                rect.right() - ITEM_PAD_X - KEBAB_SIZE.x / 2.0,
                rect.center().y,
            ),
            KEBAB_SIZE,
        );
        if renaming {
            self.show_rename_row(ui, colors, rect, entry, actions);
            return PickerItemRects {
                row: rect,
                kebab: kebab_rect,
            };
        }
        let kebab = ui.interact(kebab_rect, response.id.with("kebab"), egui::Sense::click());
        if kebab.is_pointer_button_down_on()
            && self.picker.submenu_index().is_some()
            && !self.picker_submenu_toggle_suppressed_until_release
        {
            actions.push(PickerAction::CloseSubmenuFromTrigger {
                suppress_click: submenu_open,
            });
        }
        if response.is_pointer_button_down_on()
            && !self.picker_drag_suppressed_until_release
            && !kebab.hovered()
            && !kebab.is_pointer_button_down_on()
        {
            if let Some(pointer_pos) = response.interact_pointer_pos() {
                if self.picker.start_drag_pending(index, pointer_pos, rect) {
                    self.picker_drag_animation_epoch =
                        self.picker_drag_animation_epoch.wrapping_add(1);
                    ui.ctx().request_repaint();
                }
            }
        }
        if let Some(pointer_pos) = response.interact_pointer_pos() {
            self.picker
                .promote_pending_drag(pointer_pos, DRAG_ACTIVATE_DISTANCE_SQ, entries);
        }
        let drag_active = self.picker.drag_in_progress();
        let live_reorder_active = self.picker.active_drag_index().is_some();
        let dragging_source = self.picker.drag_source_index() == Some(index);
        let pointer_hovered = (response.hovered() || kebab.hovered())
            && !drag_active
            && !self.picker_pointer_hover_suppressed(ui.ctx());
        let hovered = pointer_hovered || focused || submenu_open;
        if kebab.hovered() {
            ui.output_mut(|output| output.cursor_icon = egui::CursorIcon::PointingHand);
        } else if self.picker.drag_in_progress() {
            ui.output_mut(|output| output.cursor_icon = egui::CursorIcon::Grabbing);
        } else if pointer_hovered {
            ui.output_mut(|output| output.cursor_icon = egui::CursorIcon::PointingHand);
        }
        if !dragging_source {
            let visual_rect = self.animated_picker_item_rect(ui, entry, rect, live_reorder_active);
            if hovered && !drag_active {
                ui.painter().rect_filled(
                    visual_rect,
                    egui::CornerRadius::same(ITEM_RADIUS),
                    colors.background, // Flexoki bg-2.
                );
            }
            paint_picker_item_text(ui, colors, visual_rect, kebab_rect, entry, active, 255);
        }
        if (hovered || submenu_open) && !dragging_source && !drag_active {
            paint_kebab(
                ui.painter(),
                kebab_rect.center(),
                if kebab.hovered() || submenu_open {
                    colors.toolbar_icon_hover // Flexoki tx.
                } else {
                    colors.toolbar_icon_disabled // Flexoki tx-3.
                },
            );
        }
        if kebab.clicked() {
            if !self.picker_submenu_toggle_suppressed_until_release {
                actions.push(PickerAction::OpenSubmenu(index));
            }
        } else if response.clicked()
            && (!self.picker.drag_in_progress() || self.picker.pending_drag_index() == Some(index))
        {
            actions.push(PickerAction::Select(index));
        }
        PickerItemRects {
            row: rect,
            kebab: kebab_rect,
        }
    }

    fn show_picker_submenu(
        &self,
        ctx: &egui::Context,
        colors: &Colors,
        index: usize,
        kebab_rect: egui::Rect,
        parent_row_rect: egui::Rect,
        actions: &mut Vec<PickerAction>,
    ) -> Option<egui::Rect> {
        let viewport = ctx.content_rect();
        let row_top = parent_row_rect.top();
        let right = egui::pos2(kebab_rect.right() + SUBMENU_GAP, row_top);
        let left = egui::pos2(kebab_rect.left() - SUBMENU_WIDTH - SUBMENU_GAP, row_top);
        let pos = if right.x + SUBMENU_WIDTH > viewport.right() {
            left
        } else {
            right
        };
        let can_up = index > 0;
        let can_down = index + 1 < self.library.entries.len();
        let can_delete = self.library.entries.len() > 1;
        let area = egui::Area::new(egui::Id::new("circuit-picker-submenu"))
            .order(egui::Order::Tooltip)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                popover_frame(colors).show(ui, |ui| {
                    ui.set_min_width(SUBMENU_WIDTH - 12.0);
                    ui.set_max_width(SUBMENU_WIDTH - 12.0);
                    if submenu_item(ui, colors, "Rename", true, false).clicked() {
                        actions.push(PickerAction::StartRename(index));
                    }
                    if submenu_item(ui, colors, "Duplicate", true, false).clicked() {
                        actions.push(PickerAction::Duplicate(index));
                    }
                    if submenu_item(ui, colors, "Move up", can_up, false).clicked() {
                        actions.push(PickerAction::MoveUp(index));
                    }
                    if submenu_item(ui, colors, "Move down", can_down, false).clicked() {
                        actions.push(PickerAction::MoveDown(index));
                    }
                    paint_divider(ui, colors);
                    if submenu_item(ui, colors, "Delete", can_delete, true).clicked() {
                        actions.push(PickerAction::Delete(index));
                    }
                });
            });
        publish_picker_submenu_geometry_json(
            index,
            parent_row_rect,
            kebab_rect,
            area.response.rect,
        );
        Some(area.response.rect)
    }

    fn handle_picker_keyboard(&mut self, ctx: &egui::Context) {
        if self.picker.drag_in_progress() {
            if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
                if let Some(original_entries) = self.picker.cancel_drag() {
                    self.library.entries = original_entries;
                }
                self.picker_drag_suppressed_until_release =
                    ctx.input(|input| input.pointer.primary_down());
                ctx.request_repaint();
            }
            return;
        }
        let PickerState::Open {
            focused_index,
            focus_visible,
            submenu,
            renaming,
            ..
        } = &mut self.picker
        else {
            return;
        };
        if renaming.is_some() || self.library.entries.is_empty() {
            return;
        }
        let len = self.library.entries.len();
        let key = ctx.input(|input| {
            if input.key_pressed(egui::Key::Escape) {
                Some(egui::Key::Escape)
            } else if input.key_pressed(egui::Key::ArrowDown) {
                Some(egui::Key::ArrowDown)
            } else if input.key_pressed(egui::Key::ArrowUp) {
                Some(egui::Key::ArrowUp)
            } else if input.key_pressed(egui::Key::Enter) {
                Some(egui::Key::Enter)
            } else {
                None
            }
        });
        let mut select_index = None;
        match key {
            Some(egui::Key::Escape) if submenu.is_some() => {
                *submenu = None;
                ctx.request_repaint();
            }
            Some(egui::Key::Escape) => {
                self.picker.close();
                ctx.request_repaint();
            }
            Some(egui::Key::ArrowDown) => {
                *focus_visible = true;
                *focused_index = (*focused_index + 1) % len;
                ctx.request_repaint();
            }
            Some(egui::Key::ArrowUp) => {
                *focus_visible = true;
                *focused_index = (*focused_index + len - 1) % len;
                ctx.request_repaint();
            }
            Some(egui::Key::Enter) => select_index = Some(*focused_index),
            _ => {}
        }
        if let Some(index) = select_index {
            self.select_circuit_entry(index, ctx);
        }
    }

    fn handle_picker_outside_click(
        &mut self,
        ctx: &egui::Context,
        trigger_rect: egui::Rect,
        dropdown_rect: egui::Rect,
    ) {
        if !self.picker.is_open() || self.picker.submenu_index().is_some() {
            return;
        }
        let clicked = ctx.input(|input| {
            input.pointer.any_click()
                && input
                    .pointer
                    .interact_pos()
                    .is_some_and(|pos| !trigger_rect.contains(pos) && !dropdown_rect.contains(pos))
        });
        if clicked {
            self.picker.close();
            ctx.request_repaint();
        }
    }

    fn handle_submenu_outside_click(
        &mut self,
        ctx: &egui::Context,
        dropdown_rect: egui::Rect,
        submenu_rect: Option<egui::Rect>,
    ) {
        if self.picker.submenu_index().is_none() {
            return;
        }
        let Some(submenu_rect) = submenu_rect else {
            return;
        };
        let clicked = ctx.input(|input| {
            input.pointer.any_click()
                && input
                    .pointer
                    .interact_pos()
                    .is_some_and(|pos| !submenu_rect.contains(pos) && !dropdown_rect.contains(pos))
        });
        if clicked {
            self.picker.close_submenu();
            ctx.request_repaint();
        }
    }
}

fn picker_max_items_height(entries_len: usize, viewport_height: f32) -> f32 {
    let content_cap = entries_len as f32 * ITEM_HEIGHT + ITEMS_CONTENT_EXTRA;
    let viewport_cap = (viewport_height - ITEMS_VIEWPORT_CAP_MARGIN).max(MIN_ITEMS_HEIGHT);
    content_cap.min(viewport_cap).max(MIN_ITEMS_HEIGHT)
}
