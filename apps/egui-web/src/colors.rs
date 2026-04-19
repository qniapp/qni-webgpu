use eframe::egui;

pub(crate) struct Colors {
    pub(crate) background: egui::Color32,
    pub(crate) surface: egui::Color32,
    pub(crate) line: egui::Color32,
    pub(crate) box_fill: egui::Color32,
    pub(crate) box_border: egui::Color32,
    pub(crate) label: egui::Color32,
    pub(crate) text: egui::Color32,
    pub(crate) state_fill: egui::Color32,
    pub(crate) state_outline: egui::Color32,
    pub(crate) state_outline_zero: egui::Color32,
    pub(crate) state_needle: egui::Color32,
}

impl Colors {
    pub(crate) fn new() -> Self {
        Self {
            background: crate::color_rgba(0.976, 0.98, 0.984, 1.0),
            surface: crate::color_rgba(1.0, 1.0, 1.0, 1.0),
            line: crate::color_rgba(0.72, 0.72, 0.72, 1.0),
            box_fill: crate::color_rgba(0.2, 0.62, 0.55, 1.0),
            box_border: crate::color_rgba(0.82, 0.82, 0.82, 1.0),
            label: crate::color_rgba(1.0, 1.0, 1.0, 1.0),
            text: crate::color_rgba(0.45, 0.45, 0.45, 1.0),
            state_fill: crate::color_rgba(0.055, 0.647, 0.914, 1.0), // Tailwind sky-500: rgb(14, 165, 233)
            state_outline: crate::color_rgba(0.0, 0.0, 0.0, 1.0),
            state_outline_zero: crate::color_rgba(0.75, 0.75, 0.75, 1.0),
            state_needle: crate::color_rgba(0.0, 0.0, 0.0, 1.0),
        }
    }
}
