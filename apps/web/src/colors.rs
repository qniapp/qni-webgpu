use eframe::egui;

pub(crate) mod flexoki;

#[cfg(test)]
mod regression_tests;

/// Currently shipped visual theme. Additional themes should add another
/// `ThemeKind` variant and a sibling constructor next to `flexoki_light`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ThemeKind {
    FlexokiLight,
}

pub(crate) const DEFAULT_THEME: ThemeKind = ThemeKind::FlexokiLight;

/// Resolved application colours. Call sites consume semantic roles only; raw
/// palette tones live in `colors/flexoki.rs` so future theme switching
/// stays a data change instead of a render-code hunt.
#[derive(Clone, Copy)]
pub(crate) struct Colors {
    pub(crate) background: egui::Color32,
    pub(crate) surface: egui::Color32,
    pub(crate) line: egui::Color32,
    pub(crate) box_fill: egui::Color32,
    pub(crate) drag_fill: egui::Color32,
    pub(crate) drag_accent: egui::Color32,
    pub(crate) gate_hover_border: egui::Color32,
    pub(crate) label: egui::Color32,
    pub(crate) text: egui::Color32,
    pub(crate) text_strong: egui::Color32,
    pub(crate) state_fill: egui::Color32,
    pub(crate) sample_on_core: egui::Color32,
    pub(crate) sample_on_fill: egui::Color32,
    pub(crate) sample_on_glow: egui::Color32,
    pub(crate) sample_off_fill: egui::Color32,
    pub(crate) sample_off_edge: egui::Color32,
    pub(crate) amplitude_disk_border: egui::Color32,
    pub(crate) display_placeholder_fill: egui::Color32,
    pub(crate) state_outline: egui::Color32,
    pub(crate) state_outline_zero: egui::Color32,
    pub(crate) state_needle: egui::Color32,
    pub(crate) probability_log_hint: egui::Color32,
    pub(crate) semantic_off: egui::Color32,
    pub(crate) semantic_on: egui::Color32,
    pub(crate) semantic_intermediate: egui::Color32,
    pub(crate) semantic_disabled: egui::Color32,
    pub(crate) state_handle_bg: egui::Color32,
    pub(crate) state_resize_handle_top_idle: egui::Color32,
    pub(crate) state_resize_handle_top_drag: egui::Color32,
    pub(crate) state_resize_handle_bottom_idle: egui::Color32,
    pub(crate) state_resize_handle_bottom_drag: egui::Color32,
    pub(crate) span_resize_handle_bg: egui::Color32,
    pub(crate) span_resize_handle_bg_hover: egui::Color32,
    pub(crate) popup_icon: egui::Color32,
    pub(crate) popup_icon_chrome: egui::Color32,
    pub(crate) popover_outline: egui::Color32,
    pub(crate) bloch_sphere_bg: egui::Color32,
    pub(crate) bloch_sphere_lines: egui::Color32,
    pub(crate) bloch_vector_line: egui::Color32,
    pub(crate) bloch_vector_tip_0: egui::Color32,
    pub(crate) bloch_vector_tip_mid: egui::Color32,
    pub(crate) bloch_vector_tip_1: egui::Color32,
    pub(crate) bloch_vector_tip_outline: egui::Color32,
    pub(crate) bloch_vector_zero: egui::Color32,
    pub(crate) measurement_fired_icon: egui::Color32,
    pub(crate) spacer_dots: egui::Color32,
    pub(crate) step_preview: egui::Color32,
    pub(crate) circuit_block_fill: egui::Color32,
    pub(crate) circuit_block_border: egui::Color32,
    pub(crate) circuit_block_label: egui::Color32,
    pub(crate) circuit_block_placeholder: egui::Color32,
    pub(crate) palette_shadow: egui::Color32,
    pub(crate) state_panel_shadow: egui::Color32,
    pub(crate) state_cell_popup_shadow: egui::Color32,
    pub(crate) minimap_bg: egui::Color32,
    pub(crate) minimap_viewport_fill: egui::Color32,
    pub(crate) minimap_viewport_stroke: egui::Color32,
    pub(crate) aspect_thumb_idle: egui::Color32,
    pub(crate) aspect_text_current: egui::Color32,
    pub(crate) fps_hud_bg: egui::Color32,
    pub(crate) fps_hud_border: egui::Color32,
    pub(crate) fps_hud_text: egui::Color32,
    pub(crate) fps_graph_bg: egui::Color32,
    pub(crate) fps_axis: egui::Color32,
    pub(crate) fps_plot: egui::Color32,
    pub(crate) fps_axis_label: egui::Color32,
    pub(crate) fps_total: egui::Color32,
    pub(crate) fps_cpu: egui::Color32,
    pub(crate) fps_svp: egui::Color32,
    pub(crate) toolbar_hover_bg: egui::Color32,
    pub(crate) toolbar_icon: egui::Color32,
    pub(crate) toolbar_icon_hover: egui::Color32,
    pub(crate) toolbar_icon_disabled: egui::Color32,
    pub(crate) carousel_arrow: egui::Color32,
    pub(crate) carousel_arrow_disabled: egui::Color32,
    pub(crate) carousel_dot: egui::Color32,
    pub(crate) carousel_dot_current: egui::Color32,
    pub(crate) toolbar_shadow: egui::Color32,
    pub(crate) gpu_status_running: egui::Color32,
    pub(crate) gpu_status_completed: egui::Color32,
    pub(crate) gpu_status_failed: egui::Color32,
    pub(crate) gpu_status_separator: egui::Color32,
}

pub(crate) struct Theme {
    pub(crate) kind: ThemeKind,
    pub(crate) colors: Colors,
}

impl Theme {
    pub(crate) fn default() -> Self {
        Self::new(DEFAULT_THEME)
    }

    pub(crate) fn new(kind: ThemeKind) -> Self {
        Self {
            kind,
            colors: Colors::for_theme(kind),
        }
    }

    pub(crate) fn apply_to_context(&self, ctx: &egui::Context) {
        let mut visuals = egui::Visuals::light();
        visuals.panel_fill = self.colors.background;
        visuals.window_fill = self.colors.surface;
        visuals.faint_bg_color = self.colors.surface;
        visuals.extreme_bg_color = self.colors.background;
        visuals.hyperlink_color = self.colors.semantic_on;
        // Pale selection keeps near-black glyphs readable.
        visuals.selection.bg_fill = flexoki::BLUE_100;
        visuals.selection.stroke = egui::Stroke::NONE;
        ctx.set_visuals(visuals);
    }
}

impl Colors {
    pub(crate) fn new() -> Self {
        Self::for_theme(DEFAULT_THEME)
    }

    pub(crate) fn for_theme(theme: ThemeKind) -> Self {
        match theme {
            ThemeKind::FlexokiLight => flexoki_light(),
        }
    }
}

pub(crate) fn with_alpha(color: egui::Color32, alpha: u8) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

pub(crate) fn lerp_color(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    fn channel(a: u8, b: u8, t: f32) -> u8 {
        (a as f32 + (b as f32 - a as f32) * t).round() as u8
    }
    egui::Color32::from_rgba_unmultiplied(
        channel(a.r(), b.r(), t),
        channel(a.g(), b.g(), t),
        channel(a.b(), b.b(), t),
        channel(a.a(), b.a(), t),
    )
}

/// Strict Flexoki Light using named tones from `colors/flexoki.rs`.
fn flexoki_light() -> Colors {
    Colors {
        background: flexoki::BG_2,
        surface: flexoki::BG,
        line: flexoki::UI_2,
        box_fill: flexoki::CYAN_400,
        drag_fill: flexoki::PURPLE_600,
        drag_accent: flexoki::PURPLE_300,
        gate_hover_border: flexoki::PURPLE_400,
        label: flexoki::BG,
        text: flexoki::TX_2,
        text_strong: flexoki::TX,
        state_fill: flexoki::BLUE_200,
        sample_on_core: flexoki::BLUE_100,
        sample_on_fill: flexoki::BLUE_200,
        sample_on_glow: flexoki::BLUE_600,
        sample_off_fill: flexoki::UI_2,
        sample_off_edge: flexoki::TX_3,
        amplitude_disk_border: flexoki::BLUE_400,
        display_placeholder_fill: flexoki::PURPLE_100,
        state_outline: flexoki::TX_2,
        state_outline_zero: flexoki::UI_2,
        state_needle: flexoki::TX,
        probability_log_hint: flexoki::TX_3,
        semantic_off: flexoki::RED_600,
        semantic_on: flexoki::BLUE_600,
        semantic_intermediate: flexoki::PURPLE_600,
        semantic_disabled: flexoki::TX_2,
        state_handle_bg: flexoki::BLUE_600,
        state_resize_handle_top_idle: flexoki::BLUE_400,
        state_resize_handle_top_drag: flexoki::BG,
        state_resize_handle_bottom_idle: flexoki::UI_2,
        state_resize_handle_bottom_drag: flexoki::TX_2,
        span_resize_handle_bg: flexoki::CYAN_400,
        span_resize_handle_bg_hover: flexoki::CYAN_600,
        popup_icon: flexoki::BLUE_400,
        popup_icon_chrome: flexoki::TX_3,
        popover_outline: flexoki::TX_3,
        bloch_sphere_bg: flexoki::BG_2,
        bloch_sphere_lines: flexoki::TX_3,
        bloch_vector_line: flexoki::TX,
        bloch_vector_tip_0: flexoki::RED_300,
        bloch_vector_tip_mid: flexoki::PURPLE_400,
        bloch_vector_tip_1: flexoki::BLUE_300,
        bloch_vector_tip_outline: flexoki::TX,
        bloch_vector_zero: flexoki::TX_3,
        measurement_fired_icon: flexoki::UI_2,
        spacer_dots: flexoki::TX,
        step_preview: flexoki::BLUE_600,
        circuit_block_fill: with_alpha(flexoki::UI, 128),
        circuit_block_border: flexoki::BLUE_400,
        circuit_block_label: flexoki::BLUE_400,
        circuit_block_placeholder: flexoki::TX_3,
        palette_shadow: with_alpha(flexoki::TX, 25),
        state_panel_shadow: with_alpha(flexoki::TX, 25),
        state_cell_popup_shadow: with_alpha(flexoki::TX, 36),
        minimap_bg: with_alpha(flexoki::TX, 140),
        minimap_viewport_fill: with_alpha(flexoki::BG, 70),
        minimap_viewport_stroke: with_alpha(flexoki::BG, 220),
        aspect_thumb_idle: with_alpha(flexoki::TX_2, 180),
        aspect_text_current: with_alpha(flexoki::BG, 255),
        fps_hud_bg: with_alpha(flexoki::TX, 235),
        fps_hud_border: flexoki::TX_2,
        fps_hud_text: flexoki::BG,
        fps_graph_bg: with_alpha(flexoki::TX, 180),
        fps_axis: flexoki::TX_3,
        fps_plot: flexoki::GREEN_600,
        fps_axis_label: flexoki::TX_3,
        fps_total: flexoki::PURPLE_400,
        fps_cpu: flexoki::BLUE_300,
        fps_svp: flexoki::CYAN_400,
        toolbar_hover_bg: flexoki::UI,
        toolbar_icon: flexoki::TX_2,
        toolbar_icon_hover: flexoki::TX,
        toolbar_icon_disabled: flexoki::TX_3,
        carousel_arrow: flexoki::TX,
        carousel_arrow_disabled: flexoki::UI_2,
        carousel_dot: flexoki::UI_2,
        carousel_dot_current: flexoki::BLUE_600,
        toolbar_shadow: with_alpha(flexoki::TX, 15),
        gpu_status_running: flexoki::BLUE_600,
        gpu_status_completed: flexoki::GREEN_600,
        gpu_status_failed: flexoki::RED_600,
        gpu_status_separator: flexoki::TX_3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carousel_current_dot_uses_flexoki_blue_600() {
        assert_eq!(Colors::new().carousel_dot_current, flexoki::BLUE_600);
    }

    #[test]
    fn drag_accent_uses_flexoki_purple_300() {
        assert_eq!(Colors::new().drag_accent, flexoki::PURPLE_300);
    }

    #[test]
    fn carousel_idle_dot_uses_flexoki_ui_2() {
        assert_eq!(Colors::new().carousel_dot, flexoki::UI_2);
    }

    #[test]
    fn span_resize_handle_default_uses_flexoki_cyan_400() {
        assert_eq!(Colors::new().span_resize_handle_bg, flexoki::CYAN_400);
    }

    #[test]
    fn span_resize_handle_hover_uses_flexoki_cyan_600() {
        assert_eq!(Colors::new().span_resize_handle_bg_hover, flexoki::CYAN_600);
    }

    #[test]
    fn popover_outline_uses_flexoki_tx_3() {
        assert_eq!(Colors::new().popover_outline, flexoki::TX_3);
    }

    #[test]
    fn selection_uses_flexoki_blue_100() {
        let ctx = egui::Context::default();
        Theme::default().apply_to_context(&ctx);
        assert_eq!(
            ctx.global_style().visuals.selection.bg_fill,
            flexoki::BLUE_100
        );
    }
}
