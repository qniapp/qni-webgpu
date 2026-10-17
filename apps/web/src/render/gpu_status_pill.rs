use std::time::Duration;

use eframe::egui;

use crate::app::ExternalGpuStatus;
use crate::colors::Colors;

struct PillParts {
    color: egui::Color32,
    label: &'static str,
    detail: Option<String>,
    running: bool,
}

pub fn gpu_status_pill(
    ui: &mut egui::Ui,
    status: &ExternalGpuStatus,
    compact: bool,
) -> Option<egui::Response> {
    let colors = Colors::new();
    let parts = pill_parts(status, &colors)?;
    if compact {
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(crate::tailwind::SPACING_2, crate::tailwind::SPACING_8),
            egui::Sense::hover(),
        );
        ui.painter()
            .circle_filled(rect.center(), crate::tailwind::SPACING_1, parts.color);
        let label = match parts.detail {
            Some(detail) => format!("{} · {detail}", parts.label),
            None => parts.label.to_owned(),
        };
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &label));
        return Some(response.on_hover_text(label));
    }

    if parts.running {
        ui.ctx().request_repaint_after(Duration::from_millis(600));
    }

    let inner = egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(
            crate::tailwind::SPACING_2 as i8,
            crate::tailwind::SPACING_0_5 as i8,
        ))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(crate::tailwind::SPACING_2, 0.0);
                paint_dot(ui, parts.color, parts.running);
                ui.label(
                    egui::RichText::new(parts.label)
                        .size(crate::tailwind::TEXT_SM)
                        .color(parts.color),
                );
                if let Some(detail) = parts.detail {
                    ui.label(
                        egui::RichText::new("·")
                            .size(crate::tailwind::TEXT_SM)
                            .color(colors.gpu_status_separator),
                    );
                    ui.label(
                        egui::RichText::new(detail)
                            .monospace()
                            .size(crate::tailwind::TEXT_SM)
                            .color(parts.color),
                    );
                }
            });
        });
    Some(inner.response)
}

fn pill_parts(status: &ExternalGpuStatus, colors: &Colors) -> Option<PillParts> {
    match status {
        ExternalGpuStatus::Idle => None,
        ExternalGpuStatus::Running => Some(PillParts {
            color: colors.gpu_status_running,
            label: "Running on GPU…",
            detail: None,
            running: true,
        }),
        ExternalGpuStatus::Completed { duration } => Some(PillParts {
            color: colors.gpu_status_completed,
            label: "Completed",
            detail: Some(crate::app::format_gpu_duration(*duration)),
            running: false,
        }),
        ExternalGpuStatus::Failed(failure) => Some(PillParts {
            color: colors.gpu_status_failed,
            label: failure.label(),
            detail: Some(failure.detail()),
            running: false,
        }),
    }
}

fn paint_dot(ui: &mut egui::Ui, color: egui::Color32, running: bool) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(6.0, 6.0), // 6px status dot.
        egui::Sense::hover(),
    );
    let color = if running {
        let id = ui.make_persistent_id("gpu_status_pill_dot_pulse");
        let time = ui.ctx().input(|input| input.time);
        let high = ((time / 0.6).floor() as i64) % 2 == 0;
        let t = ui.ctx().animate_bool_with_time(id, high, 0.6);
        let alpha = (0.35 + 0.65 * t).clamp(0.35, 1.0);
        crate::colors::with_alpha(color, (255.0 * alpha).round() as u8)
    } else {
        color
    };
    ui.painter().circle_filled(rect.center(), 3.0, color); // radius 3px = 6px dot.
}
