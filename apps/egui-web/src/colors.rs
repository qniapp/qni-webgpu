use eframe::egui;

/// Application palette — strict Flexoki Light (Steph Ango / kepano,
/// <https://github.com/kepano/flexoki>). All semantic colours below map
/// to a Flexoki named tone; the comment on each line spells out which.
///
/// Flexoki Light reference tones used here:
///   bg     paper      #FFFCF0  (1.000, 0.988, 0.941)
///   bg-2   base-50    #F2F0E5  (0.949, 0.941, 0.898)
///   ui     base-100   #E6E4D9  (0.902, 0.894, 0.851)
///   ui-2   base-150   #DAD8CE  (0.855, 0.847, 0.808)
///   tx-3   base-300   #B7B5AC  (0.718, 0.710, 0.675)
///   tx-2   base-600   #6F6E69  (0.435, 0.431, 0.412)
///   tx     black      #100F0F  (0.063, 0.059, 0.059)
///   red-600           #AF3029  (0.686, 0.188, 0.161)
///   green-600         #66800B  (0.400, 0.502, 0.043)
///   cyan-400          #3AA99F  (0.227, 0.663, 0.624)
///   blue-100          #BCD1E0  (0.737, 0.820, 0.878)
///   blue-300          #66A0C8  (0.400, 0.627, 0.784)
///   blue-400          #4385BE  (0.263, 0.522, 0.745)
///   blue-600          #205EA6  (0.125, 0.369, 0.651)
///   purple-400        #8B7EC8  (0.545, 0.494, 0.784)
///   purple-600        #5E409D  (0.369, 0.251, 0.616)
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
    pub(crate) semantic_disabled: egui::Color32,
    pub(crate) state_handle_bg: egui::Color32,
    /// Top resize handle (inside the blue header strip) — idle.
    pub(crate) state_resize_handle_top_idle: egui::Color32,
    /// Top resize handle — while dragging.
    pub(crate) state_resize_handle_top_drag: egui::Color32,
    /// Bottom resize handle (on the paper panel surface) — idle.
    pub(crate) state_resize_handle_bottom_idle: egui::Color32,
    /// Bottom resize handle — while dragging.
    pub(crate) state_resize_handle_bottom_drag: egui::Color32,
    /// QFT / QFT† resize-handle chrome at the bottom of the gate body
    /// (Flexoki purple-400 idle, purple-600 on hover). The body itself
    /// uses `box_fill` (green-600) like every other unitary gate.
    pub(crate) qft_resize_handle_bg: egui::Color32,
    pub(crate) qft_resize_handle_bg_hover: egui::Color32,
    /// Accent colour for the state-cell hover popup icons (amplitude /
    /// probability / phase glyphs). qni paints these in sky-500
    /// (#0EA5E9) — Flexoki's nearest "saturated, bright blue" tone is
    /// blue-400 (#4385BE), which keeps the icons readable at 14px and
    /// still feels like an accent rather than data-ink. Reserved for
    /// the *one-point* element of each icon (amp arrow, prob inner
    /// disk, phase arc).
    pub(crate) popup_icon: egui::Color32,
    /// Chrome colour for the popup icons — the supporting strokes
    /// (amp axes + origin dot, prob outer ring, phase base + hypotenuse).
    /// Flexoki tx-3 (#B7B5AC) sits a step lighter than the label text
    /// (tx-2) so the blue accent reads as the foreground subject.
    pub(crate) popup_icon_chrome: egui::Color32,
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
            // bg-2 (base-50) — page / canvas behind the cards.
            background: crate::shared::color_rgba(0.949, 0.941, 0.898, 1.0),
            // paper — palette / state-panel surface (one notch lighter
            // than the page so cards read as "elevated").
            surface: crate::shared::color_rgba(1.000, 0.988, 0.941, 1.0),
            // ui-2 — qubit wires.
            line: crate::shared::color_rgba(0.855, 0.847, 0.808, 1.0),
            // cyan-400 — unitary gate body. Flexoki's green-600 read as
            // olive in practice; cyan-400 lands closer to qni's original
            // teal emerald-500 (#10b981) while staying on-palette.
            box_fill: crate::shared::color_rgba(0.227, 0.663, 0.624, 1.0),
            // purple-600 — gate-drag preview ("intermediate / ghost"
            // tone, same as `semantic_intermediate`).
            drag_fill: crate::shared::color_rgba(0.369, 0.251, 0.616, 1.0),
            // ui — gate hover outline.
            box_border: crate::shared::color_rgba(0.902, 0.894, 0.851, 1.0),
            // paper — gate label / icon strokes (on green/blue bodies).
            label: crate::shared::color_rgba(1.000, 0.988, 0.941, 1.0),
            // tx-2 — body text (q-labels etc.).
            text: crate::shared::color_rgba(0.435, 0.431, 0.412, 1.0),
            // blue-300 — amplitude fill in the state panel. Two
            // Flexoki steps lighter than the panel header (blue-600),
            // so the dark `state_needle` line stays high-contrast and
            // the hover brightness(0.9) darken still reads against the
            // pale base.
            state_fill: crate::shared::color_rgba(0.400, 0.627, 0.784, 1.0),
            // tx-2 — state-vector outline (non-zero amplitude). Lighter
            // than the needle so the border reads as chrome / surround
            // and the needle as the data. Matches qni's split: outline
            // slate-500, needle slate-900.
            state_outline: crate::shared::color_rgba(0.435, 0.431, 0.412, 1.0),
            // ui-2 — outline for zero-amplitude circles.
            state_outline_zero: crate::shared::color_rgba(0.855, 0.847, 0.808, 1.0),
            // tx — phase-needle line.
            state_needle: crate::shared::color_rgba(0.063, 0.059, 0.059, 1.0),
            // red-600 — |0⟩ measurement / write digit colour.
            semantic_off: crate::shared::color_rgba(0.686, 0.188, 0.161, 1.0),
            // blue-600 — |1⟩ measurement / write digit colour.
            semantic_on: crate::shared::color_rgba(0.125, 0.369, 0.651, 1.0),
            // purple-600 — intermediate / dragging ghost (= drag_fill).
            semantic_intermediate: crate::shared::color_rgba(0.369, 0.251, 0.616, 1.0),
            // tx-2 — disabled fills (write-gate brackets, control / swap
            // icons in the palette).
            semantic_disabled: crate::shared::color_rgba(0.435, 0.431, 0.412, 1.0),
            // blue-600 — state-panel header strip (the same blue as
            // `state_fill` so the strip reads as part of the panel).
            state_handle_bg: crate::shared::color_rgba(0.125, 0.369, 0.651, 1.0),
            // blue-100 — top resize handle on the blue strip; pops to
            // paper while being dragged.
            state_resize_handle_top_idle: crate::shared::color_rgba(0.737, 0.820, 0.878, 1.0),
            state_resize_handle_top_drag: crate::shared::color_rgba(1.000, 0.988, 0.941, 1.0),
            // ui-2 → tx-2 — bottom resize handle on the paper panel.
            state_resize_handle_bottom_idle: crate::shared::color_rgba(0.855, 0.847, 0.808, 1.0),
            state_resize_handle_bottom_drag: crate::shared::color_rgba(0.435, 0.431, 0.412, 1.0),
            // purple-400 / purple-600 — QFT resize handle (matches
            // qni's `--qni-component-resize-handle-fill-color{,-hovered}`
            // family, but recoloured into Flexoki's purple tones).
            qft_resize_handle_bg: crate::shared::color_rgba(0.545, 0.494, 0.784, 1.0),
            qft_resize_handle_bg_hover: crate::shared::color_rgba(0.369, 0.251, 0.616, 1.0),
            // blue-400 — popup icon accent (qni uses sky-500 #0EA5E9;
            // blue-400 #4385BE is the closest saturated-bright blue on
            // Flexoki Light).
            popup_icon: crate::shared::color_rgba(0.263, 0.522, 0.745, 1.0),
            // tx-3 (#B7B5AC) — popup icon chrome (axes / outer ring /
            // base lines). One step lighter than label text (tx-2) so
            // the blue accent stays foreground.
            popup_icon_chrome: crate::shared::color_rgba(0.718, 0.710, 0.675, 1.0),
            // bg-2 → tx-3 → tx → red-600 → blue-600 — bloch sphere
            // background, latitude / longitude lines, vector line,
            // tip and origin dot.
            bloch_sphere_bg: crate::shared::color_rgba(0.949, 0.941, 0.898, 1.0),
            bloch_sphere_lines: crate::shared::color_rgba(0.718, 0.710, 0.675, 1.0),
            bloch_vector_line: crate::shared::color_rgba(0.063, 0.059, 0.059, 1.0),
            bloch_vector_tip: crate::shared::color_rgba(0.686, 0.188, 0.161, 1.0),
            bloch_vector_zero: crate::shared::color_rgba(0.125, 0.369, 0.651, 1.0),
            // ui — "fired" meter icon (the colored digit dominates on top).
            measurement_fired_icon: crate::shared::color_rgba(0.902, 0.894, 0.851, 1.0),
            // tx — spacer ellipsis squares.
            spacer_dots: crate::shared::color_rgba(0.063, 0.059, 0.059, 1.0),
        }
    }
}
