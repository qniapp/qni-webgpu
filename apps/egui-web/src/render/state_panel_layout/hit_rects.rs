use eframe::egui;

use super::StatePanelLayout;
use crate::app::{QniApp, ResizeCorner};
use crate::colors::Colors;
use crate::constants::{STATE_PANEL_CORNER_RADIUS, STATE_RESIZE_HIT_PAD};

impl QniApp {
    /// Hit rect for the strip's dimensions text ("C × R = N states ▾"),
    /// which is wheel-scrollable for aspect ±1 and click-able to open the
    /// aspect popover. Computed by measuring the text with the actual font
    /// so the rect exactly tracks the rendered glyphs; expanded by a few
    /// px on all sides for forgiving clicks.
    pub(crate) fn dims_text(layout: &StatePanelLayout) -> String {
        let states_label = if layout.state_count == 1 {
            "state"
        } else {
            "states"
        };
        let rows = layout.state_count / layout.columns.max(1);
        format!(
            "{} × {} = {} {} ▾",
            layout.columns, rows, layout.state_count, states_label
        )
    }

    pub(crate) fn dims_hit_rect(
        ctx: &egui::Context,
        layout: &StatePanelLayout,
        state_panel_offset: egui::Vec2,
    ) -> egui::Rect {
        let state_rect = layout.state_rect.translate(state_panel_offset);
        let handle_rect = egui::Rect::from_min_size(
            state_rect.min,
            egui::vec2(state_rect.width(), layout.handle_height.max(6.0)),
        );
        let strip_padding_x = STATE_PANEL_CORNER_RADIUS + 6.0;
        // Must mirror the strip_font in state_panel_draw.rs — both must
        // sit at Tailwind text-sm (14px) for the hit-test rect to line
        // up with what the user actually sees.
        let font = egui::FontId::monospace(14.0);
        let text = Self::dims_text(layout);
        let measure_color = Colors::new().aspect_text_current;
        let size = ctx.fonts_mut(|f| f.layout_no_wrap(text, font, measure_color).size());
        let right_center = handle_rect.right_center() - egui::vec2(strip_padding_x, 0.0);
        let visible = egui::Rect::from_min_max(
            egui::pos2(right_center.x - size.x, right_center.y - size.y / 2.0),
            egui::pos2(right_center.x, right_center.y + size.y / 2.0),
        );
        visible.expand2(egui::vec2(6.0, 4.0))
    }

    /// Hit rect for grabbing one corner. The visible handle is an arc of
    /// the panel's rounded inner edge, but for clicks we expose the full
    /// `R × R` square at the corner (the panel's rounded-corner bounding
    /// box) inflated by `STATE_RESIZE_HIT_PAD` so the corner is forgiving
    /// to grab even when the cursor isn't right on the curve.
    pub(crate) fn resize_handle_hit_rect(
        layout: &StatePanelLayout,
        offset: egui::Vec2,
        corner: ResizeCorner,
    ) -> egui::Rect {
        let state_rect = layout.state_rect.translate(offset);
        let r = STATE_PANEL_CORNER_RADIUS;
        let base = match corner {
            ResizeCorner::TopLeft => egui::Rect::from_min_size(state_rect.min, egui::vec2(r, r)),
            ResizeCorner::TopRight => egui::Rect::from_min_size(
                egui::pos2(state_rect.max.x - r, state_rect.min.y),
                egui::vec2(r, r),
            ),
            ResizeCorner::BottomLeft => egui::Rect::from_min_size(
                egui::pos2(state_rect.min.x, state_rect.max.y - r),
                egui::vec2(r, r),
            ),
            ResizeCorner::BottomRight => egui::Rect::from_min_size(
                egui::pos2(state_rect.max.x - r, state_rect.max.y - r),
                egui::vec2(r, r),
            ),
        };
        base.expand(STATE_RESIZE_HIT_PAD)
    }
}
