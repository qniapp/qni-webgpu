use eframe::egui;

use super::geometry::PopupGeometry;
use crate::colors::Colors;
use crate::render::popover::{self, PopoverPlacement, PopoverTail};

pub(super) fn paint_popup_chrome(painter: &egui::Painter, colors: &Colors, popup: &PopupGeometry) {
    popover::paint_popover(
        painter,
        colors,
        PopoverPlacement {
            rect: popup.rect,
            tail: PopoverTail::from_points(
                popup.tail_base_l(),
                popup.tail_base_r(),
                popup.tail_apex(),
            ),
        },
    );
}
