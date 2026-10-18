use crate::colors::{with_alpha, Colors};
use eframe::egui;

pub(super) const DROPDOWN_WIDTH: f32 = 240.0;
pub(super) const SECTION_HEADER_HEIGHT: f32 = 26.0; // 16px line height + 10px vertical padding.
pub(super) const SECTION_HEADER_TOP_MARGIN: f32 = crate::tailwind::SPACING_1_5; // Space between sections.
pub(super) const ITEM_HEIGHT: f32 = 36.0;
pub(super) const ITEM_RADIUS: u8 = crate::tailwind::ROUNDED_MD;
pub(super) const ITEM_PAD_X: f32 = crate::tailwind::SPACING_2_5;
pub(super) const ROW_ICON_SIZE: f32 = 14.0; // mock row icon box = 14px.
pub(super) const ROW_ICON_GAP: f32 = crate::tailwind::SPACING_2;
pub(super) const ROW_TEXT_OFFSET: f32 = ITEM_PAD_X + ROW_ICON_SIZE + ROW_ICON_GAP;
pub(super) const TOPBAR_BOTTOM_OFFSET: f32 = crate::tailwind::SPACING_1_5; // Trigger bottom → topbar bottom.

pub(super) fn popover_frame(colors: &Colors) -> egui::Frame {
    egui::Frame {
        inner_margin: egui::Margin::same(crate::tailwind::SPACING_1_5 as i8),
        fill: colors.surface,
        stroke: egui::Stroke::new(1.0_f32, colors.line),
        corner_radius: egui::CornerRadius::same(crate::tailwind::ROUNDED_XL),
        outer_margin: egui::Margin::ZERO,
        shadow: egui::epaint::Shadow {
            offset: [0, 12],
            blur: 32,
            spread: 0,
            color: with_alpha(colors.text_strong, 25),
        },
    }
}

pub(super) fn paint_section_header(
    ui: &mut egui::Ui,
    colors: &Colors,
    label: &'static str,
    top_margin: bool,
) -> egui::Rect {
    if top_margin {
        ui.add_space(SECTION_HEADER_TOP_MARGIN);
    }
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), SECTION_HEADER_HEIGHT),
        egui::Sense::hover(),
    );
    let color = colors.toolbar_icon_disabled;
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        egui::FontId::new(crate::tailwind::TEXT_XS, egui::FontFamily::Proportional),
        color,
    );
    let text_pos = egui::pos2(
        rect.left() + ROW_TEXT_OFFSET,
        rect.center().y - galley.size().y / 2.0,
    );
    let line_y = rect.center().y;
    ui.painter().line_segment(
        [
            egui::pos2(rect.left() + ITEM_PAD_X, line_y),
            egui::pos2(rect.left() + ITEM_PAD_X + ROW_ICON_SIZE, line_y),
        ],
        egui::Stroke::new(1.0_f32, colors.line),
    );
    ui.painter().galley(text_pos, galley.clone(), color);
    ui.painter().line_segment(
        [
            egui::pos2(text_pos.x + galley.size().x + 6.0, line_y),
            egui::pos2(rect.right() - 4.0, line_y),
        ],
        egui::Stroke::new(1.0_f32, colors.line),
    );
    rect
}

pub(super) fn paint_divider(ui: &mut egui::Ui, colors: &Colors) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 13.0), egui::Sense::hover());
    ui.painter().line_segment(
        [
            egui::pos2(rect.left() + 4.0, rect.center().y),
            egui::pos2(rect.right() - 4.0, rect.center().y),
        ],
        egui::Stroke::new(1.0_f32, colors.line),
    );
}
