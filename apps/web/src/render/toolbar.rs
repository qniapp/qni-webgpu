use eframe::egui;

use crate::app::{ExecMode, QniApp};
use crate::colors::{with_alpha, Colors};
use crate::constants::SECTION_DIVIDER_WIDTH;

use super::gpu_status_pill::gpu_status_pill;

const TUTORIAL_URL: &str = "https://qniapp.github.io/qni-tutorial-webgpu/";
const TUTORIAL_TOOLTIP: &str = "Open tutorial in a new tab";

const TOOL_SIZE: egui::Vec2 = egui::vec2(crate::tailwind::SPACING_8, crate::tailwind::SPACING_8);
const ICON_SIZE: egui::Vec2 = egui::vec2(18.0, 18.0);

#[derive(Clone, Copy)]
enum ToolbarIcon {
    Undo,
    Redo,
    Trash,
    Copy,
    Lock,
    LockOpen,
    Play,
    More,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    S0,
    S1,
    S2,
    S3,
    S4,
}

impl Stage {
    fn overflow(self, has_toggle: bool) -> Vec<&'static str> {
        let mut items = Vec::new();
        if has_toggle && matches!(self, Self::S3 | Self::S4) {
            items.push("ExecMode");
        }
        if self != Self::S0 {
            items.push("Tutorial");
        }
        items
    }
    fn toggle_visible(self, has_toggle: bool) -> bool {
        has_toggle && !matches!(self, Self::S3 | Self::S4)
    }
}

#[derive(Clone, Copy)]
struct DensityMeasurements {
    left: f32,
    left_dot: f32,
    tutorial: f32,
    toggle: f32,
}
#[derive(Clone, Copy)]
struct ToolbarMeasurements {
    standard: DensityMeasurements,
    compact: DensityMeasurements,
}

fn density(compact: bool) -> (f32, f32, f32, f32) {
    use crate::tailwind::*;
    if compact {
        (SPACING_2, SPACING_0, SPACING_2, SPACING_6)
    } else {
        (SPACING_3, SPACING_2, SPACING_4, SPACING_8)
    }
}

fn stage_width(
    stage: Stage,
    compact: bool,
    has_toggle: bool,
    m: ToolbarMeasurements,
) -> (f32, f32) {
    let compact = compact || stage == Stage::S4;
    let (_, spacing, _, icon) = density(compact);
    let m = if compact { m.compact } else { m.standard };
    let left = if matches!(stage, Stage::S2 | Stage::S3 | Stage::S4) {
        m.left_dot
    } else {
        m.left
    };
    let mut widths = Vec::new();
    if stage == Stage::S0 {
        widths.push(m.tutorial);
    }
    if stage.toggle_visible(has_toggle) {
        widths.push(m.toggle);
    }
    if stage != Stage::S0 {
        widths.push(icon);
    }
    (
        left,
        widths.iter().sum::<f32>() + spacing * widths.len().saturating_sub(1) as f32,
    )
}

fn select_stage(width: f32, compact: bool, has_toggle: bool, m: ToolbarMeasurements) -> Stage {
    for stage in [Stage::S0, Stage::S1, Stage::S2, Stage::S3, Stage::S4] {
        if stage == Stage::S3 && !has_toggle || stage == Stage::S4 && compact {
            continue;
        }
        let (padding, _, gap, _) = density(compact || stage == Stage::S4);
        let (left, right) = stage_width(stage, compact, has_toggle, m);
        if 2.0 * padding + left + gap + right <= width {
            return stage;
        }
    }
    if compact {
        if has_toggle {
            Stage::S3
        } else {
            Stage::S2
        }
    } else {
        Stage::S4
    }
}

#[derive(Default)]
pub(crate) struct MoreState {
    pub(crate) open: bool,
    button_focus: bool,
    row_focus: Option<usize>,
}

fn menu_focus_after_arrow(index: usize, count: usize, down: bool) -> usize {
    (index + if down { 1 } else { count - 1 }) % count
}

impl QniApp {
    pub(crate) fn show_exec_mode_toolbar(&mut self, ctx: &egui::Context, colors: &Colors) {
        if !self.mode.uses_browser_state() {
            self.show_embed_toolbar(ctx, colors);
            return;
        }
        self.poll_external_gpu_run(ctx);
        clear_conditional_geometry();
        self.toolbar_more_overlay_rect = None;
        let viewport = ctx.content_rect();
        let vertical = self.orientation.is_vertical();
        let has_toggle = !cfg!(feature = "pages");
        egui::Area::new(egui::Id::new("responsive_toolbar"))
            .order(egui::Order::Foreground)
            .fixed_pos(viewport.min)
            .constrain(false)
            .show(ctx, |root| {
                // Area's remembered size is deliberately never a layout input.
                let measurements = ToolbarMeasurements {
                    standard: self.measure_toolbar(root, colors, false),
                    compact: self.measure_toolbar(root, colors, true),
                };
                let stage = select_stage(viewport.width(), vertical, has_toggle, measurements);
                let compact = vertical || stage == Stage::S4;
                let (padding, spacing, _, _) = density(compact);
                let (left_width, right_width) =
                    stage_width(stage, vertical, has_toggle, measurements);
                let toggle_visible = stage.toggle_visible(has_toggle);
                let overflow = stage.overflow(has_toggle);
                publish_overflow(stage, &overflow, toggle_visible);
                if overflow.is_empty() {
                    self.toolbar_more = MoreState::default();
                }
                if crate::app::exec_mode::take_exec_mode_focus_request() {
                    self.focus_toolbar_control(toggle_visible, !overflow.is_empty());
                }
                if !toggle_visible {
                    self.exec_mode_keyboard_focus = false;
                }
                let bar =
                    egui::Rect::from_min_size(viewport.min, egui::vec2(viewport.width(), 44.0));
                root.painter().add(
                    toolbar_frame(colors, 0.0)
                        .paint(bar.shrink2(egui::vec2(0.0, crate::tailwind::SPACING_1_5))),
                );
                root.allocate_rect(bar, egui::Sense::hover());
                let y = viewport.top() + crate::tailwind::SPACING_1_5;
                let left_rect = egui::Rect::from_min_size(
                    egui::pos2(viewport.left() + padding, y),
                    egui::vec2(left_width, TOOL_SIZE.y),
                );
                let right_rect = egui::Rect::from_min_size(
                    egui::pos2(viewport.right() - padding - right_width, y),
                    egui::vec2(right_width, TOOL_SIZE.y),
                );
                let mut left = root.new_child(
                    egui::UiBuilder::new()
                        .id("toolbar_left".into())
                        .max_rect(left_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                );
                left.spacing_mut().item_spacing = egui::vec2(spacing, 0.0);
                self.show_circuit_picker(&mut left, colors, ctx, compact);
                self.show_edit_utilities(&mut left, colors, ctx, compact);
                if self.exec_mode == ExecMode::Gpu {
                    paint_toolbar_divider(&mut left, colors);
                    self.show_gpu_execute_cluster(
                        &mut left,
                        colors,
                        ctx,
                        compact,
                        compact || matches!(stage, Stage::S2 | Stage::S3 | Stage::S4),
                    );
                }
                if self.picker.is_open() {
                    self.toolbar_more.open = false;
                }
                let mut right = root.new_child(
                    egui::UiBuilder::new()
                        .id("toolbar_right".into())
                        .max_rect(right_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                );
                right.spacing_mut().item_spacing = egui::vec2(spacing, 0.0);
                if stage == Stage::S0 {
                    tutorial_link(&mut right, colors, compact);
                }
                if toggle_visible {
                    self.show_toolbar_exec_mode_toggle(&mut right, colors, compact, None);
                }
                if !overflow.is_empty() {
                    self.show_more_button(&mut right, ctx, colors, compact, &overflow, padding);
                }
            });
    }

    fn focus_toolbar_control(&mut self, toggle_visible: bool, has_overflow: bool) {
        if toggle_visible {
            self.toolbar_more.open = false;
            self.toolbar_more.button_focus = false;
            self.toolbar_more.row_focus = None;
            self.exec_mode_keyboard_focus = true;
        } else if has_overflow {
            self.toolbar_more.open = false;
            self.toolbar_more.row_focus = None;
            self.exec_mode_keyboard_focus = false;
            self.toolbar_more.button_focus = true;
        }
    }

    fn measure_toolbar(
        &self,
        ui: &egui::Ui,
        colors: &Colors,
        compact: bool,
    ) -> DensityMeasurements {
        let (_, spacing, _, icon) = density(compact);
        let mut left = self.picker_trigger_width(ui, colors, compact) + 5.0 * icon + 5.0 * spacing;
        let mut left_dot = left;
        if self.exec_mode == ExecMode::Gpu {
            left +=
                SECTION_DIVIDER_WIDTH + 2.0 * crate::tailwind::SPACING_0_5 + icon + 2.0 * spacing;
            let status = super::gpu_status_pill::gpu_status_width(
                ui,
                self.external_gpu.view().status,
                compact,
            );
            left_dot = left;
            if status > 0.0 {
                left += spacing + status;
                left_dot += spacing + crate::tailwind::SPACING_2;
            }
        }
        DensityMeasurements {
            left,
            left_dot,
            tutorial: tutorial_width(ui, compact),
            toggle: crate::app::exec_mode::exec_mode_toggle_width(ui, colors, compact),
        }
    }

    fn show_more_button(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        colors: &Colors,
        compact: bool,
        overflow: &[&str],
        padding: f32,
    ) {
        let response = icon_button(
            ui,
            colors,
            ToolbarIcon::More,
            compact,
            ButtonState {
                enabled: true,
                toggle_on: self.toolbar_more.open || self.toolbar_more.button_focus,
            },
            "More",
        );
        let mut just_opened = false;
        if response.clicked() {
            self.toolbar_more.open = !self.toolbar_more.open;
            self.toolbar_more.button_focus = false;
            self.toolbar_more.row_focus = None;
            just_opened = self.toolbar_more.open;
        }
        if self.toolbar_more.button_focus
            && !self.toolbar_more.open
            && ctx.input(|i| {
                i.key_pressed(egui::Key::Enter)
                    || i.key_pressed(egui::Key::Space)
                    || i.key_pressed(egui::Key::ArrowDown)
            })
        {
            self.toolbar_more.open = true;
            self.toolbar_more.row_focus = Some(0);
            just_opened = true;
        }
        if self.toolbar_more.open {
            self.close_picker_popover();
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.toolbar_more.open = false;
                self.toolbar_more.button_focus = true;
                self.toolbar_more.row_focus = None;
                return;
            }
            if !just_opened {
                for (key, down) in [(egui::Key::ArrowDown, true), (egui::Key::ArrowUp, false)] {
                    if ctx.input(|i| i.key_pressed(key)) {
                        self.toolbar_more.row_focus = Some(self.toolbar_more.row_focus.map_or(
                            if down { 0 } else { overflow.len() - 1 },
                            |index| {
                                menu_focus_after_arrow(
                                    index.min(overflow.len() - 1),
                                    overflow.len(),
                                    down,
                                )
                            },
                        ));
                    }
                }
            }
            self.toolbar_more.row_focus = self
                .toolbar_more
                .row_focus
                .map(|i| i.min(overflow.len() - 1));
            let rect =
                self.show_more_menu(ctx, colors, response.rect, overflow, padding, just_opened);
            let outside = ctx.input(|i| {
                i.pointer.any_pressed()
                    && i.pointer
                        .interact_pos()
                        .is_some_and(|p| !rect.contains(p) && !response.rect.contains(p))
            });
            if outside {
                self.toolbar_more.open = false;
                self.toolbar_more.button_focus = false;
                self.toolbar_more.row_focus = None;
            }
            if self.toolbar_more.open {
                self.toolbar_more_overlay_rect = Some(rect);
            } else {
                clear_menu_geometry(overflow.contains(&"ExecMode"));
            }
        } else if ctx.input(|i| i.pointer.any_pressed()) && !response.hovered() {
            self.toolbar_more.button_focus = false;
        }
    }

    fn show_more_menu(
        &mut self,
        ctx: &egui::Context,
        colors: &Colors,
        trigger: egui::Rect,
        overflow: &[&str],
        padding: f32,
        just_opened: bool,
    ) -> egui::Rect {
        use super::toolbar_chrome::*;
        let has_mode = overflow.contains(&"ExecMode");
        let width = if has_mode {
            DROPDOWN_WIDTH
        } else {
            crate::tailwind::SPACING_44
        };
        let pos = egui::pos2(
            (trigger.right() - width).max(ctx.content_rect().left() + padding),
            trigger.bottom() + TOPBAR_BOTTOM_OFFSET,
        );
        let frame = popover_frame(colors);
        let margin = frame.total_margin();
        egui::Area::new(egui::Id::new("toolbar_more_menu"))
            .fade_in(false)
            .order(egui::Order::Tooltip)
            .fixed_pos(pos)
            .constrain(false)
            .show(ctx, |root| {
                let background = root.painter().add(egui::Shape::Noop);
                // Explicit inner rect also makes live menu-content changes history independent.
                let inner = egui::Rect::from_min_size(
                    pos + egui::vec2(margin.left, margin.top),
                    egui::vec2(width - margin.sum().x, 0.0),
                );
                let mut ui = root.new_child(
                    egui::UiBuilder::new()
                        .max_rect(inner)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                ui.spacing_mut().item_spacing.y = 0.0;
                let mut tutorial_rect = None;
                if has_mode {
                    paint_section_header(&mut ui, colors, "EXECUTION", false);
                    let (row, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), ITEM_HEIGHT),
                        egui::Sense::hover(),
                    );
                    if self.toolbar_more.row_focus == Some(0) {
                        ui.painter().rect_filled(
                            row,
                            egui::CornerRadius::same(ITEM_RADIUS),
                            colors.background,
                        );
                    }
                    let track_width = row.width() - 2.0 * crate::tailwind::SPACING_1;
                    let rect = egui::Rect::from_center_size(
                        row.center(),
                        egui::vec2(track_width, row.height()),
                    );
                    let mut toggle_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .id("overflow_exec".into())
                            .max_rect(rect)
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );
                    self.exec_mode_keyboard_focus =
                        self.toolbar_more.row_focus == Some(0) && !just_opened;
                    self.show_toolbar_exec_mode_toggle(
                        &mut toggle_ui,
                        colors,
                        false,
                        Some(track_width),
                    );
                    if !self.local_exec_mode_available() {
                        let (note, _) = ui.allocate_exact_size(
                            egui::vec2(
                                ui.available_width(),
                                crate::tailwind::LEADING_4 + 2.0 * crate::tailwind::SPACING_1,
                            ),
                            egui::Sense::hover(),
                        );
                        ui.painter().text(
                            egui::pos2(note.left() + crate::tailwind::SPACING_3, note.center().y),
                            egui::Align2::LEFT_CENTER,
                            "Local is available up to 16 qubits.",
                            egui::FontId::proportional(crate::tailwind::TEXT_XS),
                            colors.text,
                        );
                    }
                    paint_divider(&mut ui, colors);
                }
                if overflow.contains(&"Tutorial") {
                    let focused = self.toolbar_more.row_focus == Some(if has_mode { 1 } else { 0 });
                    let response = tutorial_menu_item(&mut ui, colors, focused);
                    tutorial_rect = Some(response.rect);
                    if response.clicked()
                        || !just_opened
                            && focused
                            && ctx.input(|i| {
                                i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Space)
                            })
                    {
                        ctx.open_url(egui::OpenUrl::new_tab(TUTORIAL_URL));
                        self.toolbar_more.open = false;
                        self.toolbar_more.row_focus = None;
                    }
                }
                let outer = ui.min_rect() + margin;
                root.painter().set(background, frame.paint(ui.min_rect()));
                root.allocate_rect(outer, egui::Sense::hover());
                publish_toolbar_button_debug_json("More menu", outer, false);
                if let Some(rect) = tutorial_rect {
                    publish_toolbar_button_debug_json("More Tutorial", rect, false);
                }
                outer
            })
            .inner
    }

    fn show_embed_toolbar(&mut self, ctx: &egui::Context, colors: &Colors) {
        let viewport = ctx.content_rect();
        let compact = self.orientation.is_vertical();
        let (padding_x, item_spacing) = if compact {
            (crate::tailwind::SPACING_2, crate::tailwind::SPACING_0)
        } else {
            (crate::tailwind::SPACING_3, crate::tailwind::SPACING_2)
        };
        egui::Area::new(egui::Id::new("exec_mode_toolbar"))
            .order(egui::Order::Foreground)
            .fixed_pos(viewport.min)
            .show(ctx, |ui| {
                ui.set_min_width(viewport.width());
                toolbar_frame(colors, padding_x).show(ui, |ui| {
                    ui.set_min_width((viewport.width() - 2.0 * padding_x).max(0.0));
                    ui.set_min_height(crate::tailwind::SPACING_8);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(item_spacing, 0.0);
                        self.show_edit_utilities(ui, colors, ctx, compact);
                        if self.exec_mode == ExecMode::Gpu {
                            paint_toolbar_divider(ui, colors);
                            self.show_gpu_execute_cluster(ui, colors, ctx, compact, compact);
                        }
                    });
                });
            });
    }

    #[cfg(all(test, not(target_arch = "wasm32")))]
    pub(crate) fn show_test_edit_utilities(&mut self, ui: &mut egui::Ui) {
        self.show_edit_utilities(
            ui,
            &self.colors(),
            &ui.ctx().clone(),
            self.orientation.is_vertical(),
        );
    }

    fn show_edit_utilities(
        &mut self,
        ui: &mut egui::Ui,
        colors: &Colors,
        ctx: &egui::Context,
        compact: bool,
    ) {
        let edit_allowed = !self.library.active_locked();
        if icon_button(
            ui,
            colors,
            ToolbarIcon::Undo,
            compact,
            ButtonState {
                enabled: edit_allowed && self.can_undo_circuit(),
                toggle_on: false,
            },
            "Undo",
        )
        .clicked()
        {
            self.undo_circuit(ctx);
        }
        if icon_button(
            ui,
            colors,
            ToolbarIcon::Redo,
            compact,
            ButtonState {
                enabled: edit_allowed && self.can_redo_circuit(),
                toggle_on: false,
            },
            "Redo",
        )
        .clicked()
        {
            self.redo_circuit(ctx);
        }
        if self.mode.shows_clear_button()
            && icon_button(
                ui,
                colors,
                ToolbarIcon::Trash,
                compact,
                ButtonState {
                    enabled: edit_allowed,
                    toggle_on: false,
                },
                "Clear circuit",
            )
            .clicked()
        {
            self.clear_circuit(ctx);
        }
        if !self.mode.uses_browser_state() {
            return;
        }
        if icon_button(
            ui,
            colors,
            ToolbarIcon::Copy,
            compact,
            ButtonState {
                enabled: true,
                toggle_on: false,
            },
            "Duplicate circuit",
        )
        .clicked()
        {
            self.duplicate_active_circuit(ctx);
        }
        let locked = self.library.active_locked();
        let example = matches!(
            self.library.active_kind(),
            qni_web_circuit_library_model::CircuitKind::Example
        );
        let (icon, tooltip) = if example {
            (ToolbarIcon::Lock, "Locked (sample) — duplicate to edit")
        } else if locked {
            (ToolbarIcon::Lock, "Unlock circuit")
        } else {
            (ToolbarIcon::LockOpen, "Lock circuit")
        };
        if icon_button(
            ui,
            colors,
            icon,
            compact,
            ButtonState {
                enabled: !example,
                toggle_on: locked,
            },
            tooltip,
        )
        .clicked()
        {
            self.toggle_circuit_lock();
        }
    }

    fn clear_circuit(&mut self, ctx: &egui::Context) {
        self.placed_gates.clear();
        self.circuit_blocks.clear();
        self.circuit_title.clear();
        self.write_document_title();
        self.update_qubit_count();
        self.gpu_plan.mark_dirty();
        self.external_gpu.note_clear_requested();
        self.commit_current_circuit(ctx);
    }

    fn show_gpu_execute_cluster(
        &mut self,
        ui: &mut egui::Ui,
        colors: &Colors,
        ctx: &egui::Context,
        compact: bool,
        status_compact: bool,
    ) {
        let running = self.external_gpu.view().status.is_running();
        let run = icon_button(
            ui,
            colors,
            ToolbarIcon::Play,
            compact,
            ButtonState {
                enabled: !running,
                toggle_on: false,
            },
            "Run on GPU",
        );
        if running && run.hovered() {
            ui.output_mut(|output| output.cursor_icon = egui::CursorIcon::NotAllowed);
        }
        if run.clicked() {
            self.start_external_gpu_run(ctx);
        }
        if let Some(status) = gpu_status_pill(ui, self.external_gpu.view().status, status_compact) {
            publish_toolbar_button_debug_json("GPU status", status.rect, status.hovered());
        }
    }
}

fn tutorial_width(ui: &egui::Ui, compact: bool) -> f32 {
    ui.painter()
        .layout_no_wrap(
            "Tutorial".to_owned(),
            egui::FontId::proportional(crate::tailwind::TEXT_SM),
            egui::Color32::PLACEHOLDER,
        )
        .size()
        .x
        + if compact {
            0.0
        } else {
            2.0 * crate::tailwind::SPACING_2
        }
}

fn tutorial_menu_item(ui: &mut egui::Ui, colors: &Colors, focused: bool) -> egui::Response {
    use super::toolbar_chrome::*;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), ITEM_HEIGHT),
        egui::Sense::click(),
    );
    if response.hovered() || focused {
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::same(ITEM_RADIUS),
            if response.is_pointer_button_down_on() {
                colors.toolbar_hover_bg
            } else {
                colors.background
            },
        );
    }
    let text = ui.painter().layout_no_wrap(
        "Tutorial".to_owned(),
        egui::FontId::proportional(crate::tailwind::TEXT_SM),
        colors.text_strong,
    );
    ui.painter().galley(
        egui::pos2(
            rect.left() + crate::tailwind::SPACING_3,
            rect.center().y - text.size().y / 2.0,
        ),
        text,
        colors.text_strong,
    );
    let icon = egui::Rect::from_center_size(
        egui::pos2(
            rect.right() - ITEM_PAD_X - ROW_ICON_SIZE / 2.0,
            rect.center().y,
        ),
        egui::Vec2::splat(ROW_ICON_SIZE),
    );
    let stroke = egui::Stroke::new(1.5, colors.toolbar_icon);
    paint_path(
        ui.painter(),
        icon,
        &[(15.0, 3.0), (21.0, 3.0), (21.0, 9.0)],
        false,
        stroke,
    );
    paint_path(
        ui.painter(),
        icon,
        &[(10.0, 14.0), (21.0, 3.0)],
        false,
        stroke,
    );
    paint_path(
        ui.painter(),
        icon,
        &[
            (9.0, 3.0),
            (3.0, 3.0),
            (3.0, 21.0),
            (21.0, 21.0),
            (21.0, 15.0),
        ],
        false,
        stroke,
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Link, true, "Tutorial"));
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn tutorial_link(ui: &mut egui::Ui, colors: &Colors, compact: bool) -> egui::Response {
    let text = ui
        .painter()
        .layout_job(egui::text::LayoutJob::single_section(
            "Tutorial".to_owned(),
            egui::TextFormat {
                font_id: egui::FontId::proportional(crate::tailwind::TEXT_SM),
                line_height: Some(crate::tailwind::LEADING_5),
                color: colors.toolbar_icon,
                ..Default::default()
            },
        ));
    let padding_x = if compact {
        crate::tailwind::SPACING_0
    } else {
        crate::tailwind::SPACING_2
    };
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(text.size().x + 2.0 * padding_x, crate::tailwind::SPACING_8),
        egui::Sense::click(),
    );
    let response = response
        .on_hover_text(TUTORIAL_TOOLTIP)
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let hovered = response.hovered();
    publish_toolbar_button_debug_json(TUTORIAL_TOOLTIP, rect, hovered);
    let hover_t = ui
        .ctx()
        .animate_bool_with_time(response.id.with("hover"), hovered, 0.12);
    if hover_t > 0.0 {
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::same(crate::tailwind::ROUNDED_MD),
            with_alpha(colors.toolbar_hover_bg, (255.0 * hover_t) as u8),
        );
    }
    ui.painter().galley_with_override_text_color(
        rect.center() - text.size() / 2.0,
        text,
        if hovered || response.has_focus() {
            colors.toolbar_icon_hover
        } else {
            colors.toolbar_icon
        },
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Link, true, "Tutorial"));
    if response.clicked() {
        ui.ctx().open_url(egui::OpenUrl::new_tab(TUTORIAL_URL));
    }
    response
}

fn toolbar_frame(colors: &Colors, padding_x: f32) -> egui::Frame {
    egui::Frame {
        inner_margin: egui::Margin::symmetric(padding_x as i8, crate::tailwind::SPACING_1_5 as i8),
        fill: colors.surface,
        stroke: egui::Stroke::NONE,
        corner_radius: egui::CornerRadius::ZERO,
        outer_margin: egui::Margin::ZERO,
        shadow: egui::epaint::Shadow {
            offset: [0, 4],
            blur: 24,
            spread: 0,
            color: colors.toolbar_shadow,
        },
    }
}

#[derive(Clone, Copy)]
struct ButtonState {
    enabled: bool,
    toggle_on: bool,
}

fn icon_button(
    ui: &mut egui::Ui,
    colors: &Colors,
    icon: ToolbarIcon,
    compact: bool,
    state: ButtonState,
    tooltip: &'static str,
) -> egui::Response {
    let sense = if state.enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let size = if compact {
        egui::vec2(crate::tailwind::SPACING_6, TOOL_SIZE.y)
    } else {
        TOOL_SIZE
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    let response = response.on_hover_text(tooltip);
    let hovered = response.hovered() && state.enabled;
    publish_toolbar_button_debug_json(tooltip, rect, hovered);
    let more = matches!(icon, ToolbarIcon::More);
    let focus_visible = more && response.has_focus();
    let hover_t = ui
        .ctx()
        .animate_bool_with_time(response.id.with("hover"), hovered, 0.12);
    if (state.toggle_on || focus_visible) && state.enabled {
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::same(crate::tailwind::ROUNDED_MD),
            if hovered {
                colors.line
            } else {
                colors.toolbar_hover_bg
            },
        );
    } else if hover_t > 0.0 {
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::same(crate::tailwind::ROUNDED_MD),
            with_alpha(colors.toolbar_hover_bg, (255.0 * hover_t) as u8),
        );
    }

    let color = if state.enabled {
        if hovered || more && state.toggle_on || focus_visible {
            colors.toolbar_icon_hover
        } else {
            colors.toolbar_icon
        }
    } else {
        colors.toolbar_icon_disabled
    };
    response.widget_info(|| {
        if matches!(icon, ToolbarIcon::Lock | ToolbarIcon::LockOpen) {
            egui::WidgetInfo::selected(
                egui::WidgetType::Checkbox,
                state.enabled,
                state.toggle_on,
                tooltip,
            )
        } else {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, state.enabled, tooltip)
        }
    });
    paint_icon(ui.painter(), rect, icon, color);
    response
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn publish_toolbar_button_debug_json(tooltip: &str, rect: egui::Rect, hovered: bool) {
    let target = match tooltip {
        TUTORIAL_TOOLTIP => Some(crate::test_hooks::QNI_TOOLBAR_TUTORIAL_GEOMETRY_JSON),
        "Run on GPU" => Some(crate::test_hooks::QNI_TOOLBAR_RUN_GEOMETRY_JSON),
        "GPU status" => Some(crate::test_hooks::QNI_TOOLBAR_GPU_STATUS_GEOMETRY_JSON),
        "Duplicate circuit" => Some(crate::test_hooks::QNI_TOOLBAR_DUPLICATE_GEOMETRY_JSON),
        "Lock circuit" | "Unlock circuit" | "Locked (sample) — duplicate to edit" => {
            Some(crate::test_hooks::QNI_TOOLBAR_LOCK_GEOMETRY_JSON)
        }
        "More" => Some("__qniToolbarMoreGeometryJson"),
        "More menu" => Some("__qniToolbarMoreMenuGeometryJson"),
        "More Tutorial" => Some("__qniToolbarMoreTutorialGeometryJson"),
        "Exec Local" => Some("__qniToolbarExecLocalGeometryJson"),
        "Exec GPU" => Some("__qniToolbarExecGpuGeometryJson"),
        "Circuit picker" => Some("__qniToolbarPickerGeometryJson"),
        "Undo" => Some("__qniToolbarUndoGeometryJson"),
        "Redo" => Some("__qniToolbarRedoGeometryJson"),
        "Clear circuit" => Some("__qniToolbarClearGeometryJson"),
        _ => None,
    };
    let Some(target) = target else {
        return;
    };
    let json = format!(
        "{{\"left\":{:.3},\"right\":{:.3},\"top\":{:.3},\"bottom\":{:.3},\"hovered\":{},\"tooltip\":\"{}\"}}",
        rect.left(),
        rect.right(),
        rect.top(),
        rect.bottom(),
        hovered,
        tooltip.replace('"', "\\\""),
    );
    crate::test_hooks::set_window_value(target, &wasm_bindgen::JsValue::from_str(&json));
    if hovered {
        crate::test_hooks::set_window_value(
            crate::test_hooks::QNI_TOOLBAR_TOOLTIP_TEXT,
            &wasm_bindgen::JsValue::from_str(tooltip),
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn publish_toolbar_button_debug_json(_tooltip: &str, _rect: egui::Rect, _hovered: bool) {
}

#[cfg(target_arch = "wasm32")]
fn publish_overflow(stage: Stage, overflow: &[&str], toggle_visible: bool) {
    let items = overflow
        .iter()
        .map(|s| format!("\"{s}\""))
        .collect::<Vec<_>>()
        .join(",");
    crate::test_hooks::set_window_value(
        "__qniToolbarOverflowJson",
        &wasm_bindgen::JsValue::from_str(&format!(
            "{{\"stage\":\"{stage:?}\",\"overflow\":[{items}]}}"
        )),
    );
    crate::test_hooks::set_window_value(
        "__qniToolbarFocusTarget",
        &wasm_bindgen::JsValue::from_str(if toggle_visible {
            "toggle"
        } else if !overflow.is_empty() {
            "more"
        } else {
            "none"
        }),
    );
}
#[cfg(not(target_arch = "wasm32"))]
fn publish_overflow(_: Stage, _: &[&str], _: bool) {}

#[cfg(target_arch = "wasm32")]
fn clear_menu_geometry(has_mode: bool) {
    for key in [
        "__qniToolbarMoreMenuGeometryJson",
        "__qniToolbarMoreTutorialGeometryJson",
    ] {
        crate::test_hooks::set_window_value(key, &wasm_bindgen::JsValue::from_str(""));
    }
    if has_mode {
        for key in [
            "__qniToolbarExecLocalGeometryJson",
            "__qniToolbarExecGpuGeometryJson",
        ] {
            crate::test_hooks::set_window_value(key, &wasm_bindgen::JsValue::from_str(""));
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn clear_menu_geometry(_: bool) {}

#[cfg(target_arch = "wasm32")]
fn clear_conditional_geometry() {
    clear_menu_geometry(false);
    for key in [
        "__qniToolbarMoreGeometryJson",
        "__qniToolbarTutorialGeometryJson",
        "__qniToolbarRunGeometryJson",
        "__qniToolbarGpuStatusGeometryJson",
        "__qniToolbarExecLocalGeometryJson",
        "__qniToolbarExecGpuGeometryJson",
    ] {
        crate::test_hooks::set_window_value(key, &wasm_bindgen::JsValue::from_str(""));
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn clear_conditional_geometry() {}

fn paint_toolbar_divider(ui: &mut egui::Ui, colors: &Colors) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(
            SECTION_DIVIDER_WIDTH + 2.0 * crate::tailwind::SPACING_0_5,
            24.0,
        ),
        egui::Sense::hover(),
    );
    let divider_rect = egui::Rect::from_center_size(
        rect.center(),
        egui::vec2(SECTION_DIVIDER_WIDTH, rect.height()),
    );
    ui.painter()
        .rect_filled(divider_rect, egui::CornerRadius::ZERO, colors.line);
}

fn paint_icon(painter: &egui::Painter, rect: egui::Rect, icon: ToolbarIcon, color: egui::Color32) {
    let icon_rect = egui::Rect::from_center_size(rect.center(), ICON_SIZE);
    let stroke = egui::Stroke::new(crate::tailwind::STROKE_2, color);
    match icon {
        ToolbarIcon::More => {
            for x in [5.0, 12.0, 19.0] {
                painter.circle_filled(icon_point(icon_rect, x, 12.0), 1.5, color);
            }
        }
        ToolbarIcon::Undo => {
            paint_path(
                painter,
                icon_rect,
                &[(9.0, 14.0), (4.0, 9.0), (9.0, 4.0)],
                false,
                stroke,
            );
            paint_path(
                painter,
                icon_rect,
                &[
                    (4.0, 9.0),
                    (15.0, 9.0),
                    (19.0, 11.0),
                    (19.0, 15.0),
                    (17.0, 18.0),
                    (14.0, 19.0),
                ],
                false,
                stroke,
            );
        }
        ToolbarIcon::Redo => {
            paint_path(
                painter,
                icon_rect,
                &[(15.0, 14.0), (20.0, 9.0), (15.0, 4.0)],
                false,
                stroke,
            );
            paint_path(
                painter,
                icon_rect,
                &[
                    (20.0, 9.0),
                    (9.0, 9.0),
                    (5.0, 11.0),
                    (5.0, 15.0),
                    (7.0, 18.0),
                    (10.0, 19.0),
                ],
                false,
                stroke,
            );
        }
        ToolbarIcon::Trash => {
            paint_path(
                painter,
                icon_rect,
                &[(3.0, 6.0), (21.0, 6.0)],
                false,
                stroke,
            );
            paint_path(
                painter,
                icon_rect,
                &[
                    (8.0, 6.0),
                    (8.0, 4.0),
                    (10.0, 2.0),
                    (14.0, 2.0),
                    (16.0, 4.0),
                    (16.0, 6.0),
                ],
                false,
                stroke,
            );
            paint_path(
                painter,
                icon_rect,
                &[
                    (5.0, 6.0),
                    (5.0, 20.0),
                    (7.0, 22.0),
                    (17.0, 22.0),
                    (19.0, 20.0),
                    (19.0, 6.0),
                ],
                false,
                stroke,
            );
        }
        ToolbarIcon::Copy => {
            paint_icon_rect_outline(painter, icon_rect, 9.0, 9.0, 13.0, 13.0, 2.0, stroke);
            paint_copy_back_path(painter, icon_rect, stroke);
        }
        ToolbarIcon::Lock => {
            paint_icon_rect_outline(painter, icon_rect, 3.0, 11.0, 18.0, 11.0, 2.0, stroke);
            paint_shackle_closed(painter, icon_rect, stroke);
        }
        ToolbarIcon::LockOpen => {
            paint_icon_rect_outline(painter, icon_rect, 3.0, 11.0, 18.0, 11.0, 2.0, stroke);
            paint_shackle_open(painter, icon_rect, stroke);
        }
        ToolbarIcon::Play => {
            paint_path(
                painter,
                icon_rect,
                &[(6.0, 4.0), (20.0, 12.0), (6.0, 20.0)],
                true,
                stroke,
            );
        }
    }
}

fn paint_icon_rect_outline(
    painter: &egui::Painter,
    icon_rect: egui::Rect,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
    stroke: egui::Stroke,
) {
    let rect = egui::Rect::from_min_size(
        icon_point(icon_rect, x, y),
        egui::vec2(
            icon_rect.width() * width / 24.0,
            icon_rect.height() * height / 24.0,
        ),
    );
    let radius = (icon_rect.width() * radius / 24.0).round().max(0.0) as u8;
    painter.rect_stroke(
        rect,
        egui::CornerRadius::same(radius),
        stroke,
        egui::StrokeKind::Middle,
    );
}

fn paint_copy_back_path(painter: &egui::Painter, icon_rect: egui::Rect, stroke: egui::Stroke) {
    let mut points = Vec::with_capacity(20);
    points.push(icon_point(icon_rect, 5.0, 15.0));
    points.push(icon_point(icon_rect, 4.0, 15.0));
    append_icon_arc(&mut points, icon_rect, (4.0, 13.0), 2.0, 90.0, 180.0);
    points.push(icon_point(icon_rect, 2.0, 4.0));
    append_icon_arc(&mut points, icon_rect, (4.0, 4.0), 2.0, 180.0, 270.0);
    points.push(icon_point(icon_rect, 13.0, 2.0));
    append_icon_arc(&mut points, icon_rect, (13.0, 4.0), 2.0, -90.0, 0.0);
    points.push(icon_point(icon_rect, 15.0, 5.0));
    painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
        points, stroke,
    )));
}

fn paint_shackle_closed(painter: &egui::Painter, icon_rect: egui::Rect, stroke: egui::Stroke) {
    let mut points = Vec::with_capacity(10);
    points.push(icon_point(icon_rect, 7.0, 11.0));
    points.push(icon_point(icon_rect, 7.0, 7.0));
    append_icon_arc(&mut points, icon_rect, (12.0, 7.0), 5.0, 180.0, 360.0);
    points.push(icon_point(icon_rect, 17.0, 11.0));
    painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
        points, stroke,
    )));
}

fn paint_shackle_open(painter: &egui::Painter, icon_rect: egui::Rect, stroke: egui::Stroke) {
    let mut points = Vec::with_capacity(9);
    points.push(icon_point(icon_rect, 7.0, 11.0));
    points.push(icon_point(icon_rect, 7.0, 7.0));
    append_icon_arc(&mut points, icon_rect, (12.0, 7.0), 5.0, 180.0, 348.0);
    painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
        points, stroke,
    )));
}

fn append_icon_arc(
    points: &mut Vec<egui::Pos2>,
    icon_rect: egui::Rect,
    center: (f32, f32),
    radius: f32,
    start_degrees: f32,
    end_degrees: f32,
) {
    const STEPS: usize = 6;
    for step in 1..=STEPS {
        let t = step as f32 / STEPS as f32;
        let degrees = start_degrees + (end_degrees - start_degrees) * t;
        let radians = degrees.to_radians();
        points.push(icon_point(
            icon_rect,
            center.0 + radius * radians.cos(),
            center.1 + radius * radians.sin(),
        ));
    }
}

fn paint_path(
    painter: &egui::Painter,
    icon_rect: egui::Rect,
    points: &[(f32, f32)],
    closed: bool,
    stroke: egui::Stroke,
) {
    let points = points
        .iter()
        .map(|(x, y)| icon_point(icon_rect, *x, *y))
        .collect::<Vec<_>>();
    let shape = if closed {
        egui::epaint::PathShape::closed_line(points, stroke)
    } else {
        egui::epaint::PathShape::line(points, stroke)
    };
    painter.add(egui::Shape::Path(shape));
}

fn icon_point(icon_rect: egui::Rect, x: f32, y: f32) -> egui::Pos2 {
    egui::pos2(
        icon_rect.left() + icon_rect.width() * x / 24.0,
        icon_rect.top() + icon_rect.height() * y / 24.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measured() -> ToolbarMeasurements {
        ToolbarMeasurements {
            standard: DensityMeasurements {
                left: 300.0,
                left_dot: 220.0,
                tutorial: 63.0,
                toggle: 119.0,
            },
            compact: DensityMeasurements {
                left: 208.0,
                left_dot: 208.0,
                tutorial: 47.0,
                toggle: 103.0,
            },
        }
    }

    #[test]
    fn stage_boundary_includes_exact_fit() {
        assert_eq!(select_stage(530.0, false, true, measured()), Stage::S0);
    }

    #[test]
    fn stage_boundary_below_fit_hides_tutorial() {
        assert_eq!(select_stage(529.9, false, true, measured()), Stage::S1);
    }

    #[test]
    fn status_collapses_before_toggle() {
        assert_eq!(select_stage(440.0, false, true, measured()), Stage::S2);
    }

    #[test]
    fn toggle_moves_to_overflow() {
        assert_eq!(select_stage(320.0, true, true, measured()), Stage::S3);
    }

    #[test]
    fn horizontal_last_resort_uses_compact_density() {
        assert_eq!(select_stage(280.0, false, true, measured()), Stage::S4);
    }

    #[test]
    fn pages_skips_toggle_stage() {
        assert_eq!(select_stage(280.0, false, false, measured()), Stage::S4);
    }

    #[test]
    fn shrink_history_does_not_affect_stage() {
        let _ = select_stage(1440.0, false, true, measured());
        assert_eq!(select_stage(320.0, false, true, measured()), Stage::S3);
    }

    #[test]
    fn overflow_contains_only_displaced_items() {
        assert_eq!(Stage::S3.overflow(true), vec!["ExecMode", "Tutorial"]);
    }

    #[test]
    fn status_is_not_an_overflow_item() {
        assert_eq!(Stage::S2.overflow(true), vec!["Tutorial"]);
    }

    #[test]
    fn menu_focus_wraps_up() {
        assert_eq!(menu_focus_after_arrow(0, 2, false), 1);
    }

    #[test]
    fn menu_focus_wraps_down() {
        assert_eq!(menu_focus_after_arrow(1, 2, true), 0);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn toolbar_app() -> (QniApp, egui::Context) {
        let ctx = egui::Context::default();
        let mut app = QniApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()));
        app.orientation = crate::layout::CircuitOrientation::Horizontal;
        (app, ctx)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn draw_toolbar(
        app: &mut QniApp,
        ctx: &egui::Context,
        width: f32,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 240.0),
                )),
                events,
                ..Default::default()
            },
            |_| app.show_exec_mode_toolbar(ctx, &app.colors()),
        )
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn key(key: egui::Key) -> Vec<egui::Event> {
        vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]
    }

    #[cfg(all(not(target_arch = "wasm32"), not(feature = "pages")))]
    #[test]
    fn pointer_opened_menu_down_focuses_first_row() {
        let (mut app, ctx) = toolbar_app();
        app.toolbar_more.open = true;
        draw_toolbar(&mut app, &ctx, 280.0, key(egui::Key::ArrowDown));
        assert_eq!(app.toolbar_more.row_focus, Some(0));
    }

    #[cfg(all(not(target_arch = "wasm32"), not(feature = "pages")))]
    #[test]
    fn pointer_opened_menu_up_focuses_last_row() {
        let (mut app, ctx) = toolbar_app();
        app.toolbar_more.open = true;
        draw_toolbar(&mut app, &ctx, 280.0, key(egui::Key::ArrowUp));
        assert_eq!(app.toolbar_more.row_focus, Some(1));
    }

    #[cfg(all(not(target_arch = "wasm32"), not(feature = "pages")))]
    #[test]
    fn tab_to_visible_toggle_closes_tutorial_menu() {
        let (mut app, ctx) = toolbar_app();
        app.toolbar_more.open = true;
        draw_toolbar(&mut app, &ctx, 500.0, vec![]);
        app.focus_toolbar_control(true, true);
        assert!(!app.toolbar_more.open);
    }

    #[cfg(all(not(target_arch = "wasm32"), not(feature = "pages")))]
    #[test]
    fn tab_to_more_clears_the_focused_menu_row() {
        let (mut app, _) = toolbar_app();
        app.toolbar_more.open = true;
        app.toolbar_more.row_focus = Some(1);
        app.focus_toolbar_control(false, true);
        assert_eq!(app.toolbar_more.row_focus, None);
    }

    #[cfg(all(not(target_arch = "wasm32"), not(feature = "pages")))]
    #[test]
    fn tab_to_more_closes_the_menu_before_reopening() {
        let (mut app, _) = toolbar_app();
        app.toolbar_more.open = true;
        app.toolbar_more.row_focus = Some(1);
        app.focus_toolbar_control(false, true);
        assert!(!app.toolbar_more.open);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn keyboard_opens_more_at_first_row() {
        let (mut app, ctx) = toolbar_app();
        app.toolbar_more.button_focus = true;
        draw_toolbar(&mut app, &ctx, 280.0, key(egui::Key::Enter));
        assert_eq!(app.toolbar_more.row_focus, Some(0));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn escape_restores_more_focus() {
        let (mut app, ctx) = toolbar_app();
        app.toolbar_more.open = true;
        draw_toolbar(&mut app, &ctx, 280.0, key(egui::Key::Escape));
        assert!(app.toolbar_more.button_focus);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn widening_closes_empty_overflow() {
        let (mut app, ctx) = toolbar_app();
        app.toolbar_more.open = true;
        draw_toolbar(&mut app, &ctx, 280.0, vec![]);
        draw_toolbar(&mut app, &ctx, 1440.0, vec![]);
        assert!(!app.toolbar_more.open);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn open_more_blocks_circuit_input() {
        let (mut app, ctx) = toolbar_app();
        app.toolbar_more.open = true;
        draw_toolbar(&mut app, &ctx, 280.0, vec![]);
        assert!(app
            .toolbar_more_overlay_rect
            .is_some_and(|r| r.top() == 44.0 && r.right() <= 272.0));
    }

    #[cfg(all(not(target_arch = "wasm32"), not(feature = "pages")))]
    fn input_on_gate_behind_more(press: bool, blocked: bool) -> QniApp {
        let (mut app, ctx) = toolbar_app();
        let json = r#"{"cols":[["H"]]}"#;
        app.load_circuit_json_into_editor(json, &ctx);
        app.library = crate::app::circuit_library::for_startup(json.to_owned(), true).0;
        app.orientation = crate::layout::CircuitOrientation::Vertical;
        app.exec_mode = ExecMode::Gpu;
        app.toolbar_more.open = true;
        draw_toolbar(&mut app, &ctx, 280.0, vec![]);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(280.0, 800.0));
        let content = egui::Rect::from_min_max(egui::pos2(0.0, 44.0), egui::pos2(280.0, 680.0));
        let metrics = app.circuit_layout_metrics(content);
        let gate = app
            .circuit_projection(content, 0.0)
            .to_screen(egui::pos2(metrics.slot_centers[0], metrics.line_ys[0]));
        app.toolbar_more_overlay_rect
            .filter(|rect| rect.contains(gate))
            .expect("fixture gate must sit behind the actual More popup");
        if !blocked {
            app.toolbar_more_overlay_rect = None;
        }
        let mut events = vec![egui::Event::PointerMoved(gate)];
        if press {
            events.push(egui::Event::PointerButton {
                pos: gate,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            });
        }
        ctx.begin_pass(egui::RawInput {
            screen_rect: Some(screen),
            events,
            ..Default::default()
        });
        app.handle_input(content, &ctx, screen, false);
        let _ = ctx.end_pass();
        app
    }

    #[cfg(all(not(target_arch = "wasm32"), not(feature = "pages")))]
    #[test]
    fn more_popup_press_does_not_pick_up_the_gate_behind_it() {
        let app = input_on_gate_behind_more(true, true);
        assert!(app.dragging.is_none());
    }

    #[cfg(all(not(target_arch = "wasm32"), not(feature = "pages")))]
    #[test]
    fn gate_behind_more_can_be_picked_up_without_overlay_blocking() {
        let app = input_on_gate_behind_more(true, false);
        assert!(app.dragging.is_some());
    }

    #[cfg(all(not(target_arch = "wasm32"), not(feature = "pages")))]
    #[test]
    fn more_popup_hover_does_not_hover_the_gate_behind_it() {
        let app = input_on_gate_behind_more(false, true);
        assert!(app.hovered_gate_id.is_none());
    }

    #[cfg(all(not(target_arch = "wasm32"), not(feature = "pages")))]
    #[test]
    fn gate_behind_more_can_be_hovered_without_overlay_blocking() {
        let app = input_on_gate_behind_more(false, false);
        assert_eq!(app.hovered_gate_id, Some(app.placed_gates[0].id));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn picker_open_closes_more() {
        let (mut app, ctx) = toolbar_app();
        app.toolbar_more.open = true;
        app.picker = crate::app::circuit_picker_state::PickerState::open(0);
        draw_toolbar(&mut app, &ctx, 280.0, vec![]);
        assert!(!app.toolbar_more.open);
    }

    #[cfg(all(not(target_arch = "wasm32"), not(feature = "pages")))]
    #[test]
    fn mode_keyboard_selection_keeps_menu_open() {
        let (mut app, ctx) = toolbar_app();
        app.toolbar_more.open = true;
        app.toolbar_more.row_focus = Some(0);
        draw_toolbar(&mut app, &ctx, 280.0, key(egui::Key::ArrowRight));
        assert!(app.toolbar_more.open);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn tutorial_keyboard_selection_closes_menu() {
        let (mut app, ctx) = toolbar_app();
        app.toolbar_more.open = true;
        app.toolbar_more.row_focus = Some(if cfg!(feature = "pages") { 0 } else { 1 });
        draw_toolbar(&mut app, &ctx, 280.0, key(egui::Key::Enter));
        assert!(!app.toolbar_more.open);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn tutorial_keyboard_selection_opens_new_tab() {
        let (mut app, ctx) = toolbar_app();
        app.toolbar_more.open = true;
        app.toolbar_more.row_focus = Some(if cfg!(feature = "pages") { 0 } else { 1 });
        let output = draw_toolbar(&mut app, &ctx, 280.0, key(egui::Key::Enter));
        assert!(output.platform_output.commands.iter().any(|c| matches!(c, egui::OutputCommand::OpenUrl(url) if url.url == TUTORIAL_URL && url.new_tab)));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn overflow_menu_snapshot() {
        let (mut app, ctx) = toolbar_app();
        app.toolbar_more.open = true;
        draw_toolbar(&mut app, &ctx, 280.0, vec![]);
        let output = draw_toolbar(&mut app, &ctx, 280.0, vec![]);
        let texts: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) => Some(t.galley.job.text.clone()),
                _ => None,
            })
            .collect();
        insta::assert_debug_snapshot!(
            if cfg!(feature = "pages") {
                "pages_overflow_menu"
            } else {
                "abci_overflow_menu"
            },
            texts
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn actual_placement_is_independent_of_resize_history() {
        let (mut resized, ctx) = toolbar_app();
        draw_toolbar(&mut resized, &ctx, 1440.0, vec![]);
        draw_toolbar(&mut resized, &ctx, 1440.0, vec![]);
        draw_toolbar(&mut resized, &ctx, 600.0, vec![]);
        let shrunk = draw_toolbar(&mut resized, &ctx, 600.0, vec![]);
        let (mut fresh, fresh_ctx) = toolbar_app();
        draw_toolbar(&mut fresh, &fresh_ctx, 600.0, vec![]);
        let initial = draw_toolbar(&mut fresh, &fresh_ctx, 600.0, vec![]);
        let positions = |output: egui::FullOutput| {
            output
                .shapes
                .into_iter()
                .filter_map(|s| match s.shape {
                    egui::Shape::Text(t) => Some((t.galley.job.text.clone(), t.pos)),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(positions(shrunk), positions(initial));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn outside_press_closes_menu() {
        let (mut app, ctx) = toolbar_app();
        app.toolbar_more.open = true;
        draw_toolbar(&mut app, &ctx, 280.0, vec![]);
        let position = egui::pos2(4.0, 200.0);
        draw_toolbar(
            &mut app,
            &ctx,
            280.0,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        assert!(!app.toolbar_more.open);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn menu_frame_snapshot() {
        let (mut app, ctx) = toolbar_app();
        let mut draw = || {
            ctx.run_ui(Default::default(), |_| {
                app.show_more_menu(
                    &ctx,
                    &Colors::new(),
                    egui::Rect::from_min_size(egui::pos2(248.0, 6.0), egui::vec2(24.0, 32.0)),
                    &["ExecMode", "Tutorial"],
                    8.0,
                    false,
                );
            })
        };
        draw();
        let output = draw();
        insta::assert_debug_snapshot!(output.shapes);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn keyboard_opens_more() {
        let (mut app, ctx) = toolbar_app();
        app.toolbar_more.button_focus = true;
        draw_toolbar(&mut app, &ctx, 280.0, key(egui::Key::Enter));
        assert!(app.toolbar_more.open);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn escape_closes_more() {
        let (mut app, ctx) = toolbar_app();
        app.toolbar_more.open = true;
        draw_toolbar(&mut app, &ctx, 280.0, key(egui::Key::Escape));
        assert!(!app.toolbar_more.open);
    }

    #[cfg(all(not(target_arch = "wasm32"), not(feature = "pages")))]
    #[test]
    fn mode_keyboard_selection_changes_mode() {
        let (mut app, ctx) = toolbar_app();
        app.toolbar_more.open = true;
        app.toolbar_more.row_focus = Some(0);
        draw_toolbar(&mut app, &ctx, 280.0, key(egui::Key::ArrowRight));
        assert_eq!(app.exec_mode, ExecMode::Gpu);
    }

    #[cfg(all(not(target_arch = "wasm32"), not(feature = "pages")))]
    #[test]
    fn tab_after_escape_does_not_reopen_more_on_enter() {
        let (mut app, ctx) = toolbar_app();
        app.toolbar_more.open = true;
        draw_toolbar(&mut app, &ctx, 500.0, key(egui::Key::Escape));
        app.focus_toolbar_control(true, true);
        draw_toolbar(&mut app, &ctx, 500.0, key(egui::Key::Enter));
        assert!(!app.toolbar_more.open);
    }

    #[cfg(all(not(target_arch = "wasm32"), not(feature = "pages")))]
    #[test]
    fn tab_after_escape_keeps_enter_routed_to_toggle() {
        let (mut app, ctx) = toolbar_app();
        app.toolbar_more.open = true;
        draw_toolbar(&mut app, &ctx, 500.0, key(egui::Key::Escape));
        app.focus_toolbar_control(true, true);
        draw_toolbar(&mut app, &ctx, 500.0, key(egui::Key::Enter));
        assert_eq!(app.exec_mode, ExecMode::Gpu);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn execution_row_shows_keyboard_focus_on_open() {
        let (mut app, ctx) = toolbar_app();
        app.toolbar_more.row_focus = Some(0);
        let mut draw = || {
            ctx.run_ui(Default::default(), |_| {
                app.show_more_menu(
                    &ctx,
                    &Colors::new(),
                    egui::Rect::from_min_size(egui::pos2(248.0, 6.0), egui::vec2(24.0, 32.0)),
                    &["ExecMode", "Tutorial"],
                    8.0,
                    true,
                );
            })
        };
        draw();
        let output = draw();
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect) if rect.fill == Colors::new().background && rect.rect.height() == super::super::toolbar_chrome::ITEM_HEIGHT)));
    }

    #[cfg(all(not(target_arch = "wasm32"), not(feature = "pages")))]
    fn unavailable_local_app() -> (QniApp, egui::Context) {
        let (mut app, ctx) = toolbar_app();
        app.load_circuit_json_into_editor(
            r#"{"cols":[[1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,"H"]]}"#,
            &ctx,
        );
        app.toolbar_more.open = true;
        app.toolbar_more.row_focus = Some(0);
        (app, ctx)
    }

    #[cfg(all(not(target_arch = "wasm32"), not(feature = "pages")))]
    #[test]
    fn unavailable_local_cannot_be_selected_in_menu() {
        let (mut app, ctx) = unavailable_local_app();
        draw_toolbar(&mut app, &ctx, 280.0, key(egui::Key::ArrowLeft));
        assert_eq!(app.exec_mode, ExecMode::Gpu);
    }

    #[cfg(all(not(target_arch = "wasm32"), not(feature = "pages")))]
    #[test]
    fn sixteen_qubit_local_can_be_clicked_in_menu() {
        let (mut app, ctx) = toolbar_app();
        app.load_circuit_json_into_editor(
            r#"{"cols":[[1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,"H"]]}"#,
            &ctx,
        );
        app.exec_mode = ExecMode::Gpu;
        app.toolbar_more.open = true;
        draw_toolbar(&mut app, &ctx, 280.0, vec![]);
        draw_toolbar(&mut app, &ctx, 280.0, vec![]);
        let position = egui::pos2(80.0, 95.0);
        for pressed in [true, false] {
            draw_toolbar(
                &mut app,
                &ctx,
                280.0,
                vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        assert_eq!(app.exec_mode, ExecMode::Local);
    }

    #[cfg(all(not(target_arch = "wasm32"), not(feature = "pages")))]
    #[test]
    fn unavailable_local_cannot_be_clicked_in_menu() {
        let (mut app, ctx) = unavailable_local_app();
        draw_toolbar(&mut app, &ctx, 280.0, vec![]);
        draw_toolbar(&mut app, &ctx, 280.0, vec![]);
        let position = egui::pos2(80.0, 95.0);
        for pressed in [true, false] {
            draw_toolbar(
                &mut app,
                &ctx,
                280.0,
                vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        assert_eq!(app.exec_mode, ExecMode::Gpu);
    }

    #[cfg(all(not(target_arch = "wasm32"), not(feature = "pages")))]
    #[test]
    fn unavailable_local_menu_snapshot() {
        let (mut app, ctx) = unavailable_local_app();
        let mut draw = || {
            ctx.run_ui(Default::default(), |_| {
                app.show_more_menu(
                    &ctx,
                    &Colors::new(),
                    egui::Rect::from_min_size(egui::pos2(248.0, 6.0), egui::vec2(24.0, 32.0)),
                    &["ExecMode", "Tutorial"],
                    8.0,
                    false,
                );
            })
        };
        draw();
        let output = draw();
        insta::assert_debug_snapshot!(output.shapes);
    }

    #[test]
    fn pages_boundary_includes_exact_fit() {
        assert_eq!(select_stage(403.0, false, false, measured()), Stage::S0);
    }

    #[test]
    fn pages_boundary_below_fit_hides_tutorial() {
        assert_eq!(select_stage(402.9, false, false, measured()), Stage::S1);
    }

    #[test]
    fn compact_boundary_includes_exact_fit() {
        assert_eq!(select_stage(382.0, true, true, measured()), Stage::S0);
    }

    #[test]
    fn compact_boundary_below_fit_hides_tutorial() {
        assert_eq!(select_stage(381.9, true, true, measured()), Stage::S1);
    }

    #[test]
    fn idle_status_does_not_create_an_extra_stage() {
        let mut m = measured();
        m.standard.left_dot = m.standard.left;
        assert_eq!(select_stage(498.9, false, true, m), Stage::S3);
    }

    #[test]
    fn long_failure_status_collapses_before_toggle() {
        let mut m = measured();
        m.standard.left = 700.0;
        assert_eq!(select_stage(768.0, false, true, m), Stage::S2);
    }

    #[test]
    fn s1_boundary_includes_exact_fit() {
        assert_eq!(select_stage(499.0, false, true, measured()), Stage::S1);
    }

    #[test]
    fn below_s1_boundary_collapses_status() {
        assert_eq!(select_stage(498.9, false, true, measured()), Stage::S2);
    }

    #[test]
    fn s2_boundary_includes_exact_fit() {
        assert_eq!(select_stage(419.0, false, true, measured()), Stage::S2);
    }

    #[test]
    fn below_s2_boundary_hides_toggle() {
        assert_eq!(select_stage(418.9, false, true, measured()), Stage::S3);
    }

    #[test]
    fn s3_boundary_includes_exact_fit() {
        assert_eq!(select_stage(292.0, false, true, measured()), Stage::S3);
    }

    #[test]
    fn below_s3_boundary_uses_compact_density() {
        assert_eq!(select_stage(291.9, false, true, measured()), Stage::S4);
    }

    #[test]
    fn compact_s1_boundary_includes_exact_fit() {
        assert_eq!(select_stage(359.0, true, true, measured()), Stage::S1);
    }

    #[test]
    fn below_compact_s1_boundary_hides_toggle() {
        assert_eq!(select_stage(358.9, true, true, measured()), Stage::S3);
    }

    #[test]
    fn pages_s1_boundary_includes_exact_fit() {
        assert_eq!(select_stage(372.0, false, false, measured()), Stage::S1);
    }

    #[test]
    fn below_pages_s1_boundary_collapses_status() {
        assert_eq!(select_stage(371.9, false, false, measured()), Stage::S2);
    }

    #[test]
    fn more_icon_snapshot() {
        let ctx = egui::Context::default();
        let output = ctx.run_ui(Default::default(), |ui| {
            icon_button(
                ui,
                &Colors::new(),
                ToolbarIcon::More,
                true,
                ButtonState {
                    enabled: true,
                    toggle_on: false,
                },
                "More",
            );
        });
        insta::assert_debug_snapshot!(output.shapes);
    }

    #[test]
    fn tutorial_link_snapshot() {
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            tutorial_link(ui, &Colors::new(), false);
        });
        insta::assert_debug_snapshot!(output.shapes);
    }

    #[test]
    fn tutorial_click_requests_a_new_tab() {
        let ctx = egui::Context::default();
        let mut rect = egui::Rect::NOTHING;
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            rect = tutorial_link(ui, &Colors::new(), false).rect;
        });
        let position = rect.center();
        let mut output = egui::FullOutput::default();
        for pressed in [true, false] {
            output = ctx.run_ui(
                egui::RawInput {
                    events: vec![
                        egui::Event::PointerMoved(position),
                        egui::Event::PointerButton {
                            pos: position,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| {
                    tutorial_link(ui, &Colors::new(), false);
                },
            );
        }
        let urls: Vec<_> = output
            .platform_output
            .commands
            .iter()
            .filter_map(|command| match command {
                egui::OutputCommand::OpenUrl(url) => Some((url.url.as_str(), url.new_tab)),
                _ => None,
            })
            .collect();
        assert_eq!(
            urls,
            vec![("https://qniapp.github.io/qni-tutorial-webgpu/", true)]
        );
    }
}
