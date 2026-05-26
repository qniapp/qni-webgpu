use eframe::egui;

pub(super) const TRIGGER_HEIGHT: f32 = 32.0; // h-8.
pub(super) const TRIGGER_PAD_LEFT: f32 = 10.0; // px-2.5 = 10px; ITEM_PAD_X と揃える。
pub(super) const TRIGGER_PAD_RIGHT: f32 = 8.0; // spacing-2.
pub(super) const TRIGGER_NAME_CHEVRON_GAP: f32 = 6.0; // spacing-1.5.
pub(super) const TRIGGER_CHEVRON_W: f32 = 14.0; // mock chevron-down 描画幅 = 14px.
pub(super) const TRIGGER_NAME_MAX_WIDTH: f32 = 176.0; // max-w-44 = 176px — 超えたら ellipsis。
pub(super) const DROPDOWN_WIDTH: f32 = 240.0;
pub(super) const MIN_ITEMS_HEIGHT: f32 = 72.0; // 2 picker rows × 36px, mock §03 min.
pub(super) const ITEMS_VIEWPORT_CAP_MARGIN: f32 = 240.0; // Mock §03 viewport chrome/footer cap.
pub(super) const ITEMS_CONTENT_PADDING_Y: f32 = 6.0; // py-1.5 = 6px.
pub(super) const ITEMS_CONTENT_EXTRA: f32 = ITEMS_CONTENT_PADDING_Y * 2.0; // p-1.5 top+bottom = 12px, mock §03 content cap.
pub(super) const RESIZE_HANDLE_H: f32 = 8.0; // spacing-2 separator hit area.
pub(super) const RESIZE_HANDLE_MARGIN_TOP_Y: f32 = 0.0; // mt-0.
pub(super) const RESIZE_HANDLE_MARGIN_BOTTOM_Y: f32 = 2.0; // mb-0.5 = 2px.
pub(super) const RESIZE_HANDLE_LINE_INSET_X: f32 = 4.0; // spacing-1.
pub(super) const RESIZE_HANDLE_IDLE_STROKE: f32 = 1.0;
pub(super) const RESIZE_HANDLE_ACTIVE_STROKE: f32 = 2.0;
pub(super) const ITEMS_SCROLLBAR_W: f32 = 6.0; // spacing-1.5 fixed floating scrollbar width.
pub(super) const ITEMS_SCROLLBAR_OUTER_MARGIN: f32 = 2.0; // spacing-0.5 right inset.
pub(super) const ITEMS_SCROLLBAR_THUMB_RADIUS: u8 = 3; // fixed 3px thumb radius per scrollbar spec.
pub(super) const ITEMS_SCROLLBAR_IDLE_ALPHA: u8 = 153; // 60% alpha.
pub(super) const ITEMS_SCROLLBAR_HOVER_ALPHA: u8 = 179; // 70% alpha.
pub(super) const ITEMS_SCROLLBAR_FADE_SECONDS: f32 = 0.15; // 150ms opacity transition.
pub(super) const SUBMENU_WIDTH: f32 = 160.0;
pub(super) const SECTION_HEADER_HEIGHT: f32 = 26.0; // text-xs line-height 16px + py-1.25 ≈ 26px.
pub(super) const SECTION_HEADER_TOP_MARGIN: f32 = 6.0; // mt-1.5 = 6px between sections.
pub(super) const ITEM_HEIGHT: f32 = 36.0;
pub(super) const ITEM_RADIUS: u8 = 6; // rounded-md = 6px.
pub(super) const ITEM_PAD_X: f32 = 10.0; // px-2.5 = 10px.
pub(super) const ROW_ICON_SIZE: f32 = 14.0; // mock row icon box = 14px.
pub(super) const ROW_ICON_GAP: f32 = 8.0; // gap-2 = 8px.
pub(super) const ROW_TEXT_OFFSET: f32 = ITEM_PAD_X + ROW_ICON_SIZE + ROW_ICON_GAP;
pub(super) const TOPBAR_BOTTOM_OFFSET: f32 = 6.0; // py-1.5 = 6px; trigger bottom → topbar bottom.
pub(super) const SUBMENU_GAP: f32 = 4.0; // spacing-1.
pub(super) const KEBAB_SIZE: egui::Vec2 = egui::vec2(20.0, 20.0);
pub(super) const DRAG_ACTIVATE_DISTANCE_SQ: f32 = 16.0; // 4px threshold squared, per mock §02.
pub(super) const EDGE_AUTO_SCROLL_ZONE: f32 = 36.0; // h-9 = 36px edge zone, mock §03.
pub(super) const EDGE_AUTO_SCROLL_MAX_PX: f32 = 12.0; // 12px/frame max speed, mock §03.
pub(super) const LIVE_REORDER_ANIM_SECONDS: f32 = 0.08; // 80ms FLIP slide per mock §04.

#[derive(Clone, Copy, Debug)]
pub(super) struct PickerItemRects {
    pub(super) row: egui::Rect,
    pub(super) kebab: egui::Rect,
}
