pub(super) use super::super::toolbar_chrome::*;
use eframe::egui;

pub(super) const TRIGGER_HEIGHT: f32 = crate::tailwind::SPACING_8;
pub(super) const TRIGGER_PAD_LEFT: f32 = crate::tailwind::SPACING_2_5;
pub(super) const TRIGGER_PAD_RIGHT: f32 = crate::tailwind::SPACING_2;
pub(super) const TRIGGER_NAME_CHEVRON_GAP: f32 = crate::tailwind::SPACING_1_5;
pub(super) const TRIGGER_CHEVRON_W: f32 = 14.0;
pub(super) const TRIGGER_NAME_MAX_WIDTH: f32 = crate::tailwind::SPACING_44;
pub(super) const MIN_ITEMS_HEIGHT: f32 = 72.0;
pub(super) const ITEMS_VIEWPORT_CAP_MARGIN: f32 = 240.0;
pub(super) const ITEMS_CONTENT_PADDING_Y: f32 = crate::tailwind::SPACING_1_5;
pub(super) const ITEMS_CONTENT_EXTRA: f32 = ITEMS_CONTENT_PADDING_Y * 2.0;
pub(super) const RESIZE_HANDLE_H: f32 = crate::tailwind::SPACING_2;
pub(super) const RESIZE_HANDLE_MARGIN_TOP_Y: f32 = crate::tailwind::SPACING_0;
pub(super) const RESIZE_HANDLE_MARGIN_BOTTOM_Y: f32 = crate::tailwind::SPACING_0_5;
pub(super) const RESIZE_HANDLE_LINE_INSET_X: f32 = crate::tailwind::SPACING_1;
pub(super) const RESIZE_HANDLE_IDLE_STROKE: f32 = 1.0;
pub(super) const RESIZE_HANDLE_ACTIVE_STROKE: f32 = 2.0;
pub(super) const ITEMS_SCROLLBAR_W: f32 = crate::tailwind::SPACING_1_5;
pub(super) const ITEMS_SCROLLBAR_OUTER_MARGIN: f32 = crate::tailwind::SPACING_0_5;
pub(super) const ITEMS_SCROLLBAR_THUMB_RADIUS: u8 = 3;
pub(super) const ITEMS_SCROLLBAR_IDLE_ALPHA: u8 = 153;
pub(super) const ITEMS_SCROLLBAR_HOVER_ALPHA: u8 = 179;
pub(super) const ITEMS_SCROLLBAR_FADE_SECONDS: f32 = 0.15;
pub(super) const SUBMENU_WIDTH: f32 = 160.0;
pub(super) const SUBMENU_GAP: f32 = crate::tailwind::SPACING_1;
pub(super) const KEBAB_SIZE: egui::Vec2 = egui::vec2(20.0, 20.0);
pub(super) const DRAG_ACTIVATE_DISTANCE_SQ: f32 = 16.0;
pub(super) const EDGE_AUTO_SCROLL_ZONE: f32 = crate::tailwind::SPACING_9;
pub(super) const EDGE_AUTO_SCROLL_MAX_PX: f32 = 12.0;
pub(super) const LIVE_REORDER_ANIM_SECONDS: f32 = 0.08;

#[derive(Clone, Copy, Debug)]
pub(super) struct PickerItemRects {
    pub(super) row: egui::Rect,
    pub(super) kebab: egui::Rect,
}
