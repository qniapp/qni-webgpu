use eframe::egui;

pub(crate) struct Colors {
    pub(crate) background: egui::Color32,
    pub(crate) surface: egui::Color32,
    pub(crate) line: egui::Color32,
    pub(crate) box_fill: egui::Color32,
    pub(crate) drag_fill: egui::Color32,
    pub(crate) box_border: egui::Color32,
    pub(crate) label: egui::Color32,
    pub(crate) text: egui::Color32,
    pub(crate) state_fill: egui::Color32,
    pub(crate) state_outline: egui::Color32,
    pub(crate) state_outline_zero: egui::Color32,
    pub(crate) state_needle: egui::Color32,
    pub(crate) semantic_off: egui::Color32,
    pub(crate) semantic_on: egui::Color32,
    pub(crate) semantic_intermediate: egui::Color32,
    pub(crate) bloch_sphere_bg: egui::Color32,
    pub(crate) bloch_sphere_lines: egui::Color32,
    pub(crate) bloch_vector_line: egui::Color32,
    pub(crate) bloch_vector_tip: egui::Color32,
    pub(crate) bloch_vector_zero: egui::Color32,
    pub(crate) measurement_fired_icon: egui::Color32,
    pub(crate) spacer_dots: egui::Color32,
}

impl Colors {
    pub(crate) fn new() -> Self {
        Self {
            background: crate::shared::color_rgba(0.976, 0.98, 0.984, 1.0),
            surface: crate::shared::color_rgba(1.0, 1.0, 1.0, 1.0),
            line: crate::shared::color_rgba(0.72, 0.72, 0.72, 1.0),
            box_fill: crate::shared::color_rgba(0.2, 0.62, 0.55, 1.0),
            drag_fill: crate::shared::color_rgba(0.659, 0.333, 0.969, 1.0), // qni purple-500: #a855f7
            box_border: crate::shared::color_rgba(0.82, 0.82, 0.82, 1.0),
            label: crate::shared::color_rgba(1.0, 1.0, 1.0, 1.0),
            text: crate::shared::color_rgba(0.45, 0.45, 0.45, 1.0),
            state_fill: crate::shared::color_rgba(0.055, 0.647, 0.914, 1.0), // Tailwind sky-500: rgb(14, 165, 233)
            state_outline: crate::shared::color_rgba(0.0, 0.0, 0.0, 1.0),
            state_outline_zero: crate::shared::color_rgba(0.75, 0.75, 0.75, 1.0),
            state_needle: crate::shared::color_rgba(0.0, 0.0, 0.0, 1.0),
            // qni semantic-color-off (red-500: #ef4444) for |0>
            semantic_off: crate::shared::color_rgba(0.937, 0.267, 0.267, 1.0),
            // qni semantic-color-on (blue-500: #3b82f6) for |1>
            semantic_on: crate::shared::color_rgba(0.231, 0.510, 0.965, 1.0),
            // qni semantic-color-intermediate (purple-500: #a855f7); shared with drag_fill.
            semantic_intermediate: crate::shared::color_rgba(0.659, 0.333, 0.969, 1.0),
            // qni bloch-display palette: bg green-50 (#f0fdf4), gray-400 (#9ca3af),
            // gray-900 (#111827), red-500 (#ef4444), blue-500 (#3b82f6).
            bloch_sphere_bg: crate::shared::color_rgba(0.941, 0.992, 0.957, 1.0),
            bloch_sphere_lines: crate::shared::color_rgba(0.612, 0.639, 0.686, 1.0),
            bloch_vector_line: crate::shared::color_rgba(0.067, 0.094, 0.153, 1.0),
            bloch_vector_tip: crate::shared::color_rgba(0.937, 0.267, 0.267, 1.0),
            bloch_vector_zero: crate::shared::color_rgba(0.231, 0.510, 0.965, 1.0),
            // qni measurement fired icon is text-zinc-200 (#e4e4e7) so the
            // colored digit dominates.
            measurement_fired_icon: crate::shared::color_rgba(0.894, 0.894, 0.910, 1.0),
            // qni text-neutral-900 (#171717) for the spacer ellipsis squares.
            spacer_dots: crate::shared::color_rgba(0.090, 0.090, 0.090, 1.0),
        }
    }
}
