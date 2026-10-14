use super::flexoki_light;
use eframe::egui;

macro_rules! old_role {
    ($field:ident, $r:expr, $g:expr, $b:expr, $a:expr) => {
        #[test]
        fn $field() {
            let expected = egui::Color32::from_rgba_unmultiplied(
                ($r * 255.0_f32).round() as u8,
                ($g * 255.0_f32).round() as u8,
                ($b * 255.0_f32).round() as u8,
                $a,
            );
            assert_eq!(flexoki_light().$field.to_array(), expected.to_array());
        }
    };
}

old_role!(background, 0.949, 0.941, 0.898, 255);
old_role!(surface, 1.000, 0.988, 0.941, 255);
old_role!(line, 0.855, 0.847, 0.808, 255);
old_role!(box_fill, 0.227, 0.663, 0.624, 255);
old_role!(drag_fill, 0.369, 0.251, 0.616, 255);
old_role!(gate_hover_border, 0.545, 0.494, 0.784, 255);
old_role!(label, 1.000, 0.988, 0.941, 255);
old_role!(text, 0.435, 0.431, 0.412, 255);
old_role!(text_strong, 0.063, 0.059, 0.059, 255);
old_role!(state_fill, 0.573, 0.749, 0.859, 255);
old_role!(amplitude_disk_border, 0.263, 0.522, 0.745, 255);
old_role!(display_placeholder_fill, 0.925, 0.882, 0.953, 255);
old_role!(state_outline, 0.435, 0.431, 0.412, 255);
old_role!(state_outline_zero, 0.855, 0.847, 0.808, 255);
old_role!(state_needle, 0.063, 0.059, 0.059, 255);
old_role!(probability_log_hint, 0.718, 0.710, 0.675, 255);
old_role!(semantic_off, 0.686, 0.188, 0.161, 255);
old_role!(semantic_on, 0.125, 0.369, 0.651, 255);
old_role!(semantic_intermediate, 0.369, 0.251, 0.616, 255);
old_role!(semantic_disabled, 0.435, 0.431, 0.412, 255);
old_role!(state_handle_bg, 0.125, 0.369, 0.651, 255);
old_role!(state_resize_handle_top_idle, 0.263, 0.522, 0.745, 255);
old_role!(state_resize_handle_top_drag, 1.000, 0.988, 0.941, 255);
old_role!(state_resize_handle_bottom_idle, 0.855, 0.847, 0.808, 255);
old_role!(state_resize_handle_bottom_drag, 0.435, 0.431, 0.412, 255);
old_role!(span_resize_handle_bg, 0.227, 0.663, 0.624, 255);
old_role!(span_resize_handle_bg_hover, 0.141, 0.514, 0.482, 255);
old_role!(popup_icon, 0.263, 0.522, 0.745, 255);
old_role!(popup_icon_chrome, 0.718, 0.710, 0.675, 255);
old_role!(popover_outline, 0.718, 0.710, 0.675, 255);
old_role!(bloch_sphere_bg, 0.949, 0.941, 0.898, 255);
old_role!(bloch_sphere_lines, 0.718, 0.710, 0.675, 255);
old_role!(bloch_vector_line, 0.063, 0.059, 0.059, 255);
old_role!(bloch_vector_tip_0, 0.910, 0.439, 0.373, 255);
old_role!(bloch_vector_tip_mid, 0.545, 0.494, 0.784, 255);
old_role!(bloch_vector_tip_1, 0.400, 0.627, 0.784, 255);
old_role!(bloch_vector_tip_outline, 0.063, 0.059, 0.059, 255);
old_role!(bloch_vector_zero, 0.718, 0.710, 0.675, 255);
old_role!(measurement_fired_icon, 0.855, 0.847, 0.808, 255);
old_role!(spacer_dots, 0.063, 0.059, 0.059, 255);
old_role!(step_preview, 0.125, 0.369, 0.651, 255);
old_role!(circuit_block_fill, 0.902, 0.894, 0.851, 128);
old_role!(circuit_block_border, 0.263, 0.522, 0.745, 255);
old_role!(circuit_block_label, 0.263, 0.522, 0.745, 255);
old_role!(palette_shadow, 0.063, 0.059, 0.059, 25);
old_role!(state_panel_shadow, 0.063, 0.059, 0.059, 25);
old_role!(state_cell_popup_shadow, 0.063, 0.059, 0.059, 36);
old_role!(minimap_bg, 0.063, 0.059, 0.059, 140);
old_role!(minimap_viewport_fill, 1.000, 0.988, 0.941, 70);
old_role!(minimap_viewport_stroke, 1.000, 0.988, 0.941, 220);
old_role!(aspect_thumb_idle, 0.435, 0.431, 0.412, 180);
old_role!(aspect_text_current, 1.000, 0.988, 0.941, 255);
old_role!(fps_hud_bg, 0.063, 0.059, 0.059, 235);
old_role!(fps_hud_border, 0.435, 0.431, 0.412, 255);
old_role!(fps_hud_text, 1.000, 0.988, 0.941, 255);
old_role!(fps_graph_bg, 0.063, 0.059, 0.059, 180);
old_role!(fps_axis, 0.718, 0.710, 0.675, 255);
old_role!(fps_plot, 0.400, 0.502, 0.043, 255);
old_role!(fps_axis_label, 0.718, 0.710, 0.675, 255);
old_role!(fps_total, 0.545, 0.494, 0.784, 255);
old_role!(fps_cpu, 0.400, 0.627, 0.784, 255);
old_role!(fps_svp, 0.227, 0.663, 0.624, 255);
old_role!(toolbar_hover_bg, 0.902, 0.894, 0.851, 255);
old_role!(toolbar_icon, 0.435, 0.431, 0.412, 255);
old_role!(toolbar_icon_hover, 0.063, 0.059, 0.059, 255);
old_role!(toolbar_icon_disabled, 0.718, 0.710, 0.675, 255);
old_role!(toolbar_shadow, 0.063, 0.059, 0.059, 15);
old_role!(gpu_status_running, 0.125, 0.369, 0.651, 255);
old_role!(gpu_status_completed, 0.400, 0.502, 0.043, 255);
old_role!(gpu_status_failed, 0.686, 0.188, 0.161, 255);
old_role!(gpu_status_separator, 0.718, 0.710, 0.675, 255);
