use eframe::egui;

use crate::app::{ExecMode, ExternalGpuStatus, QniApp};
use crate::colors::{with_alpha, Colors};

use super::gpu_status_pill::gpu_status_pill;

const TOOL_SIZE: egui::Vec2 = egui::vec2(28.0, 24.0); // w-7 / h-6 = 28px / 24px.
const ICON_SIZE: egui::Vec2 = egui::vec2(14.0, 14.0); // text-sm icon box = 14px.

#[derive(Clone, Copy)]
enum ToolbarIcon {
    Undo,
    Redo,
    Trash,
    Play,
}

impl QniApp {
    pub(crate) fn show_exec_mode_toolbar(&mut self, ctx: &egui::Context, colors: &Colors) {
        self.poll_external_gpu_run(ctx);
        let viewport = ctx.content_rect();
        egui::Area::new(egui::Id::new("exec_mode_toolbar"))
            .order(egui::Order::Foreground)
            .fixed_pos(viewport.min)
            .show(ctx, |ui| {
                ui.set_min_width(viewport.width());
                toolbar_frame(colors).show(ui, |ui| {
                    ui.set_min_width((viewport.width() - 24.0).max(0.0)); // px-3 x both sides.
                    ui.set_min_height(24.0); // h-6 = 24px content row.
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0); // gap-2 = 8px.
                        self.show_edit_utilities(ui, colors, ctx);
                        if self.exec_mode == ExecMode::Gpu {
                            paint_toolbar_divider(ui, colors);
                            self.show_gpu_execute_cluster(ui, colors, ctx);
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            self.show_exec_mode_toggle(ui, colors);
                        });
                    });
                });
            });
    }

    fn show_edit_utilities(&mut self, ui: &mut egui::Ui, colors: &Colors, ctx: &egui::Context) {
        if icon_button(
            ui,
            colors,
            ToolbarIcon::Undo,
            self.can_undo_circuit(),
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
            self.can_redo_circuit(),
            "Redo",
        )
        .clicked()
        {
            self.redo_circuit(ctx);
        }
        if icon_button(ui, colors, ToolbarIcon::Trash, true, "Clear circuit").clicked() {
            self.placed_gates.clear();
            self.update_qubit_count();
            self.gpu_plan.mark_dirty();
            self.external_gpu_status = ExternalGpuStatus::Idle;
            self.commit_current_circuit(ctx);
        }
    }

    fn show_gpu_execute_cluster(
        &mut self,
        ui: &mut egui::Ui,
        colors: &Colors,
        ctx: &egui::Context,
    ) {
        let running = self.external_gpu_status().is_running();
        let run = icon_button(ui, colors, ToolbarIcon::Play, !running, "Run on GPU");
        if running && run.hovered() {
            ui.output_mut(|output| output.cursor_icon = egui::CursorIcon::NotAllowed);
        }
        if run.clicked() {
            self.start_external_gpu_run(ctx);
        }
        let _ = gpu_status_pill(ui, self.external_gpu_status());
    }
}

fn toolbar_frame(colors: &Colors) -> egui::Frame {
    egui::Frame {
        inner_margin: egui::Margin::symmetric(12, 6), // px-3 / py-1.5 = 12px / 6px.
        fill: colors.surface,                         // Flexoki bg / paper.
        stroke: egui::Stroke::NONE,
        corner_radius: egui::CornerRadius::ZERO, // Full-width header strip: all 4 corners square.
        outer_margin: egui::Margin::ZERO,
        shadow: egui::epaint::Shadow {
            offset: [0, 4],
            blur: 24,
            spread: 0,
            color: colors.toolbar_shadow, // Flexoki tx alpha 15.
        },
    }
}

fn icon_button(
    ui: &mut egui::Ui,
    colors: &Colors,
    icon: ToolbarIcon,
    enabled: bool,
    tooltip: &'static str,
) -> egui::Response {
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(TOOL_SIZE, sense);
    let response = response.on_hover_text(tooltip);
    let hovered = response.hovered() && enabled;
    let hover_t = ui
        .ctx()
        .animate_bool_with_time(response.id.with("hover"), hovered, 0.12);
    if hover_t > 0.0 {
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::same(6), // rounded-md = 6px.
            with_alpha(colors.toolbar_hover_bg, (255.0 * hover_t) as u8), // Flexoki ui.
        );
    }

    let color = if enabled {
        if hovered {
            colors.toolbar_icon_hover // Flexoki tx.
        } else {
            colors.toolbar_icon // Flexoki tx-2.
        }
    } else {
        colors.toolbar_icon_disabled // Flexoki tx-3.
    };
    paint_icon(ui.painter(), rect, icon, color);
    response
}

fn paint_toolbar_divider(ui: &mut egui::Ui, colors: &Colors) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(5.0, 24.0), // 1px line + mx-0.5 = 2px each side.
        egui::Sense::hover(),
    );
    let center_x = rect.center().x;
    ui.painter().line_segment(
        [
            egui::pos2(center_x, rect.center().y - 9.0),
            egui::pos2(center_x, rect.center().y + 9.0),
        ],
        egui::Stroke::new(1.0, colors.line), // Flexoki ui-2.
    );
}

fn paint_icon(painter: &egui::Painter, rect: egui::Rect, icon: ToolbarIcon, color: egui::Color32) {
    let icon_rect = egui::Rect::from_center_size(rect.center(), ICON_SIZE);
    let stroke = egui::Stroke::new(1.5, color); // Lucide 24×24 stroke-2 scaled to 14px.
    match icon {
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
