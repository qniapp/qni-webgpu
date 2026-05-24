use eframe::egui;

use crate::colors::Colors;
use crate::render::popover::{self, PopoverPlacement, PopoverTail};

pub(super) const PAD_X: f32 = 16.0; // px-4
pub(super) const PAD_Y: f32 = 12.0; // py-3

#[derive(Clone, Copy)]
pub(super) struct TooltipCard {
    pub(super) placement: PopoverPlacement,
}

pub(super) fn place_tooltip_card(
    screen_rect: egui::Rect,
    gate_rect: egui::Rect,
    content_size: egui::Vec2,
) -> TooltipCard {
    let card_size = egui::vec2(content_size.x + PAD_X * 2.0, content_size.y + PAD_Y * 2.0);
    let target_gap = popover::ANCHOR_GAP + popover::TAIL_H;
    let prefer_below = gate_rect.bottom() + target_gap + card_size.y
        <= screen_rect.bottom() - popover::VIEWPORT_PAD;
    let raw_left = gate_rect.center().x - card_size.x * 0.5;
    let raw_top = if prefer_below {
        gate_rect.bottom() + target_gap
    } else {
        gate_rect.top() - target_gap - card_size.y
    };

    let min_left = screen_rect.left() + popover::VIEWPORT_PAD;
    let max_left = screen_rect.right() - popover::VIEWPORT_PAD - card_size.x;
    let min_top = screen_rect.top() + popover::VIEWPORT_PAD;
    let max_top = screen_rect.bottom() - popover::VIEWPORT_PAD - card_size.y;
    let card_min = egui::pos2(
        raw_left.clamp(min_left, max_left.max(min_left)),
        raw_top.clamp(min_top, max_top.max(min_top)),
    );
    let rect = egui::Rect::from_min_size(card_min, card_size);
    let tail = if prefer_below {
        PopoverTail::on_top_edge(rect, gate_rect.center().x)
    } else {
        PopoverTail::on_bottom_edge(rect, gate_rect.center().x)
    };

    TooltipCard {
        placement: PopoverPlacement { rect, tail },
    }
}

pub(super) fn paint_tooltip_card(painter: &egui::Painter, card: TooltipCard, colors: &Colors) {
    popover::paint_popover(painter, colors, card.placement);
}
