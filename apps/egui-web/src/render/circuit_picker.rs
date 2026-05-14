use eframe::egui;

use crate::app::circuit_library::{CircuitEntry, PickerState};
use crate::app::QniApp;
use crate::colors::{with_alpha, Colors};

const TRIGGER_SIZE: egui::Vec2 = egui::vec2(220.0, 32.0); // max-w-[220px] / h-8.
const TRIGGER_NAME_WIDTH: f32 = 180.0;
const DROPDOWN_WIDTH: f32 = 240.0;
const DROPDOWN_MAX_HEIGHT: f32 = 320.0;
const SUBMENU_WIDTH: f32 = 160.0;
const ITEM_HEIGHT: f32 = 36.0;
const ITEM_RADIUS: u8 = 6; // rounded-md = 6px.
const ITEM_PAD_X: f32 = 10.0; // px-2.5 = 10px.
const POPOVER_GAP: f32 = 12.0; // spacing-3.
const SUBMENU_GAP: f32 = 4.0; // spacing-1.
const KEBAB_SIZE: egui::Vec2 = egui::vec2(20.0, 20.0);

#[derive(Clone, Debug)]
enum PickerAction {
    Select(usize),
    Create,
    OpenSubmenu(usize),
    StartRename(usize),
    UpdateRenameDraft { entry_id: String, draft: String },
    CommitRename { entry_id: String, draft: String },
    CancelRename,
    Duplicate(usize),
    MoveUp(usize),
    MoveDown(usize),
    Delete(usize),
}

impl QniApp {
    pub(crate) fn show_circuit_picker(
        &mut self,
        ui: &mut egui::Ui,
        colors: &Colors,
        ctx: &egui::Context,
    ) {
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
            self.picker = PickerState::open(self.library.active_index());
            ctx.request_repaint();
        }
        if let Some(dropdown_rect) = self.show_picker_dropdown(ctx, colors, trigger.rect) {
            self.handle_picker_outside_click(ctx, trigger.rect, dropdown_rect);
        }
    }

    fn show_picker_trigger(&self, ui: &mut egui::Ui, colors: &Colors) -> egui::Response {
        let (rect, mut response) = ui.allocate_exact_size(TRIGGER_SIZE, egui::Sense::click());
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
            TRIGGER_NAME_WIDTH,
            font,
        );
        ui.painter().galley(
            egui::pos2(
                rect.left() + ITEM_PAD_X,
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
        let chev_center = egui::pos2(rect.left() + 210.0, rect.center().y);
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
        let pos = trigger_rect.left_bottom() + egui::vec2(0.0, POPOVER_GAP);
        let entries = self.library.entries.clone();
        let active_id = self.library.active_id.clone();
        let focused_index = self.picker.focused_index();
        let submenu_index = self.picker.submenu_index();
        let renaming_id = self.picker.renaming_id().map(str::to_owned);
        let mut submenu_anchor = None;
        let mut actions = Vec::new();
        let area = egui::Area::new(egui::Id::new("circuit-picker-dropdown"))
            .order(egui::Order::Tooltip)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                popover_frame(colors).show(ui, |ui| {
                    ui.set_min_width(DROPDOWN_WIDTH - 12.0);
                    ui.set_max_width(DROPDOWN_WIDTH - 12.0);
                    egui::ScrollArea::vertical()
                        .max_height(DROPDOWN_MAX_HEIGHT)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            for (index, entry) in entries.iter().enumerate() {
                                let kebab_rect = self.show_picker_item(
                                    ui,
                                    colors,
                                    entry,
                                    index,
                                    entry.id == active_id,
                                    focused_index == Some(index),
                                    submenu_index == Some(index),
                                    renaming_id.as_deref() == Some(entry.id.as_str()),
                                    &mut actions,
                                );
                                if submenu_index == Some(index) {
                                    submenu_anchor = Some(kebab_rect);
                                }
                            }
                            paint_divider(ui, colors);
                            if footer(ui, colors).clicked() {
                                actions.push(PickerAction::Create);
                            }
                        });
                });
            });
        let dropdown_rect = area.response.rect;
        let submenu_rect = if let (Some(index), Some(anchor)) = (submenu_index, submenu_anchor) {
            self.show_picker_submenu(ctx, colors, index, anchor, &mut actions)
        } else {
            None
        };
        let picker_rect = submenu_rect
            .map(|rect| dropdown_rect.union(rect))
            .unwrap_or(dropdown_rect);
        for action in actions {
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
        actions: &mut Vec<PickerAction>,
    ) -> egui::Rect {
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), ITEM_HEIGHT),
            egui::Sense::click(),
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
            return kebab_rect;
        }
        let kebab = ui.interact(kebab_rect, response.id.with("kebab"), egui::Sense::click());
        let hovered = response.hovered() || kebab.hovered() || focused || submenu_open;
        if active {
            ui.painter().rect_filled(
                rect,
                egui::CornerRadius::same(ITEM_RADIUS),
                colors.toolbar_hover_bg, // Flexoki ui.
            );
        } else if hovered {
            ui.painter().rect_filled(
                rect,
                egui::CornerRadius::same(ITEM_RADIUS),
                colors.background, // Flexoki bg-2.
            );
        }
        let font = egui::FontId::new(14.0, egui::FontFamily::Proportional); // text-sm = 14px.
        let name_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left() + ITEM_PAD_X, rect.top()),
            egui::pos2(kebab_rect.left() - 8.0, rect.bottom()),
        );
        let galley = egui::WidgetText::from(
            egui::RichText::new(entry.name.clone())
                .font(font.clone())
                .color(colors.text_strong),
        )
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            name_rect.width(),
            font,
        );
        ui.painter().galley(
            egui::pos2(
                name_rect.left(),
                name_rect.center().y - galley.size().y / 2.0,
            ),
            galley,
            colors.text_strong,
        );
        if hovered || submenu_open {
            if kebab.hovered() || submenu_open {
                ui.painter().rect_filled(
                    kebab_rect,
                    egui::CornerRadius::same(4),
                    colors.toolbar_hover_bg, // Flexoki ui.
                );
            }
            paint_kebab(
                ui.painter(),
                kebab_rect.center(),
                if kebab.hovered() || submenu_open {
                    colors.toolbar_icon_hover
                } else {
                    colors.toolbar_icon_disabled
                },
            );
        }
        if kebab.clicked() {
            actions.push(PickerAction::OpenSubmenu(index));
        } else if response.clicked() {
            actions.push(PickerAction::Select(index));
        }
        kebab_rect
    }

    fn show_rename_row(
        &mut self,
        ui: &mut egui::Ui,
        colors: &Colors,
        rect: egui::Rect,
        entry: &CircuitEntry,
        actions: &mut Vec<PickerAction>,
    ) {
        ui.painter().rect(
            rect,
            egui::CornerRadius::same(ITEM_RADIUS),
            colors.surface,
            egui::Stroke::new(1.5, colors.semantic_on), // Flexoki blue-600.
            egui::StrokeKind::Outside,
        );
        let (mut draft, select_all) = match &mut self.picker {
            PickerState::Open {
                renaming: Some(rename),
                ..
            } if rename.entry_id == entry.id => {
                let select_all = rename.select_all_pending;
                rename.select_all_pending = false;
                (rename.draft.clone(), select_all)
            }
            _ => (entry.name.clone(), false),
        };
        let edit_rect = rect.shrink2(egui::vec2(ITEM_PAD_X, 0.0));
        let text_id = ui.make_persistent_id(("circuit-picker-rename", &entry.id));
        let mut edit_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("rename-child", &entry.id))
                .max_rect(edit_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        edit_ui.set_min_size(edit_rect.size());
        let output = egui::TextEdit::singleline(&mut draft)
            .id(text_id)
            .font(egui::FontId::new(14.0, egui::FontFamily::Proportional)) // text-sm = 14px.
            .frame(false)
            .desired_width(edit_rect.width())
            .show(&mut edit_ui);
        if select_all {
            output.response.request_focus();
            let mut state = output.state;
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::default(),
                    egui::text::CCursor::new(draft.chars().count()),
                )));
            state.store(ui.ctx(), output.response.id);
        }
        let (enter, escape) = ui.input(|input| {
            input
                .events
                .iter()
                .fold((false, false), |(enter, escape), event| {
                    if let egui::Event::Key {
                        key, pressed: true, ..
                    } = event
                    {
                        (
                            enter || *key == egui::Key::Enter,
                            escape || *key == egui::Key::Escape,
                        )
                    } else {
                        (enter, escape)
                    }
                })
        });
        if escape {
            actions.push(PickerAction::CancelRename);
        } else if enter || output.response.lost_focus() {
            actions.push(PickerAction::CommitRename {
                entry_id: entry.id.clone(),
                draft,
            });
        } else {
            actions.push(PickerAction::UpdateRenameDraft {
                entry_id: entry.id.clone(),
                draft,
            });
        }
    }

    fn show_picker_submenu(
        &self,
        ctx: &egui::Context,
        colors: &Colors,
        index: usize,
        kebab_rect: egui::Rect,
        actions: &mut Vec<PickerAction>,
    ) -> Option<egui::Rect> {
        let viewport = ctx.content_rect();
        let right = kebab_rect.right_top() + egui::vec2(SUBMENU_GAP, 0.0);
        let left = kebab_rect.left_top() - egui::vec2(SUBMENU_WIDTH + SUBMENU_GAP, 0.0);
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
        Some(area.response.rect)
    }

    fn handle_picker_keyboard(&mut self, ctx: &egui::Context) {
        let PickerState::Open {
            focused_index,
            submenu,
            renaming,
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
                *focused_index = (*focused_index + 1) % len;
                ctx.request_repaint();
            }
            Some(egui::Key::ArrowUp) => {
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

    fn apply_picker_action(&mut self, action: PickerAction, ctx: &egui::Context) {
        match action {
            PickerAction::Select(index) => self.select_circuit_entry(index, ctx),
            PickerAction::Create => self.create_new_circuit(ctx),
            PickerAction::OpenSubmenu(index) => self.picker.open_submenu(index),
            PickerAction::StartRename(index) => self.start_circuit_rename(index),
            PickerAction::UpdateRenameDraft { entry_id, draft } => {
                self.picker.update_rename_draft(&entry_id, draft);
            }
            PickerAction::CommitRename { entry_id, draft } => {
                self.commit_circuit_rename(&entry_id, draft);
            }
            PickerAction::CancelRename => self.picker.cancel_rename(),
            PickerAction::Duplicate(index) => self.duplicate_circuit_entry(index, ctx),
            PickerAction::MoveUp(index) => self.move_circuit_entry_up(index),
            PickerAction::MoveDown(index) => self.move_circuit_entry_down(index),
            PickerAction::Delete(index) => self.delete_circuit_entry(index, ctx),
        }
        ctx.request_repaint();
    }
}

fn popover_frame(colors: &Colors) -> egui::Frame {
    egui::Frame {
        inner_margin: egui::Margin::same(6),         // p-1.5 = 6px.
        fill: colors.surface,                        // Flexoki bg / paper.
        stroke: egui::Stroke::new(1.0, colors.line), // Flexoki ui-2.
        corner_radius: egui::CornerRadius::same(12), // rounded-xl = 12px.
        outer_margin: egui::Margin::ZERO,
        shadow: egui::epaint::Shadow {
            offset: [0, 12],
            blur: 32,
            spread: 0,
            color: with_alpha(colors.text_strong, 25),
        },
    }
}

fn footer(ui: &mut egui::Ui, colors: &Colors) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), ITEM_HEIGHT),
        egui::Sense::click(),
    );
    if response.hovered() {
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::same(ITEM_RADIUS),
            colors.background, // Flexoki bg-2.
        );
    }
    let color = if response.hovered() {
        colors.text_strong
    } else {
        colors.text
    };
    let galley = ui.painter().layout_no_wrap(
        "Create new circuit".to_owned(),
        egui::FontId::new(14.0, egui::FontFamily::Proportional), // text-sm = 14px.
        color,
    );
    ui.painter().galley(
        egui::pos2(
            rect.left() + ITEM_PAD_X,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        color,
    );
    paint_plus(
        ui.painter(),
        egui::pos2(rect.right() - ITEM_PAD_X - 7.0, rect.center().y),
        if response.hovered() {
            colors.text_strong
        } else {
            colors.toolbar_icon_disabled
        },
    );
    response
}

fn submenu_item(
    ui: &mut egui::Ui,
    colors: &Colors,
    label: &'static str,
    enabled: bool,
    destructive: bool,
) -> egui::Response {
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), ITEM_HEIGHT), sense);
    if response.hovered() && enabled {
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::same(ITEM_RADIUS),
            if destructive {
                with_alpha(colors.semantic_off, 20)
            } else {
                colors.background
            },
        );
    }
    let color = if !enabled {
        colors.toolbar_icon_disabled
    } else if destructive {
        colors.semantic_off
    } else {
        colors.text_strong
    };
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        egui::FontId::new(14.0, egui::FontFamily::Proportional), // text-sm = 14px.
        color,
    );
    ui.painter().galley(
        egui::pos2(rect.left() + 12.0, rect.center().y - galley.size().y / 2.0), // px-3 = 12px.
        galley,
        color,
    );
    if !enabled && response.hovered() {
        ui.output_mut(|output| output.cursor_icon = egui::CursorIcon::NotAllowed);
    }
    response
}

fn paint_divider(ui: &mut egui::Ui, colors: &Colors) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 13.0), egui::Sense::hover());
    ui.painter().line_segment(
        [
            egui::pos2(rect.left() + 4.0, rect.center().y),
            egui::pos2(rect.right() - 4.0, rect.center().y),
        ],
        egui::Stroke::new(1.0, colors.line),
    );
}

fn paint_chevron(painter: &egui::Painter, center: egui::Pos2, open_t: f32, color: egui::Color32) {
    let angle = std::f32::consts::PI * open_t;
    let p0 = center + rotate(egui::vec2(-4.0, -2.0), angle);
    let p1 = center + rotate(egui::vec2(0.0, 2.0), angle);
    let p2 = center + rotate(egui::vec2(4.0, -2.0), angle);
    painter.line_segment([p0, p1], egui::Stroke::new(1.8, color));
    painter.line_segment([p1, p2], egui::Stroke::new(1.8, color));
}

fn paint_kebab(painter: &egui::Painter, center: egui::Pos2, color: egui::Color32) {
    for y in [-4.0, 0.0, 4.0] {
        painter.circle_filled(center + egui::vec2(0.0, y), 1.2, color);
    }
}

fn paint_plus(painter: &egui::Painter, center: egui::Pos2, color: egui::Color32) {
    let stroke = egui::Stroke::new(1.8, color);
    painter.line_segment(
        [
            center + egui::vec2(-4.5, 0.0),
            center + egui::vec2(4.5, 0.0),
        ],
        stroke,
    );
    painter.line_segment(
        [
            center + egui::vec2(0.0, -4.5),
            center + egui::vec2(0.0, 4.5),
        ],
        stroke,
    );
}

fn rotate(point: egui::Vec2, angle: f32) -> egui::Vec2 {
    let (sin, cos) = angle.sin_cos();
    egui::vec2(point.x * cos - point.y * sin, point.x * sin + point.y * cos)
}
