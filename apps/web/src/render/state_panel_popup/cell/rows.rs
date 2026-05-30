use eframe::egui;

use super::super::icons::{draw_amplitude_icon, draw_phase_icon, draw_probability_icon};
use super::geometry::PopupGeometry;
use crate::colors::Colors;

pub(super) fn paint_popup_rows(
    painter: &egui::Painter,
    colors: &Colors,
    popup: &PopupGeometry,
    display_index: u32,
    qubits: u32,
) {
    paint_header(painter, colors, popup, display_index, qubits);
    paint_metric_rows(painter, colors, popup);
}

fn paint_header(
    painter: &egui::Painter,
    colors: &Colors,
    popup: &PopupGeometry,
    display_index: u32,
    qubits: u32,
) {
    // Label uses the cell's *display position* (= `display_index`). With the
    // q0=LSB (little-endian) convention this is also the state-vector index, so
    // the big-endian ket string `|b_{n-1}..b_0⟩` puts q0 at the right edge —
    // matching Quirk. Matches qni's `circle-notation-element.ts:907` where
    // `ket = col + row * colCount` — the same `(row, col)` the user hovers. The
    // fragment shader reads `state[display_index]` directly (no bit-reverse), so
    // the displayed numbers stay consistent with the cell the user points at.
    let ket_binary = format!("{:0width$b}", display_index, width = qubits as usize);
    let header = format!("|{}⟩ decimal {}", ket_binary, display_index);
    let header_font = egui::FontId::monospace(14.0); // text-sm = 14px
    painter.text(
        popup.header_pos(),
        egui::Align2::LEFT_TOP,
        header,
        header_font,
        colors.state_needle,
    );
}

fn paint_metric_rows(painter: &egui::Painter, colors: &Colors, popup: &PopupGeometry) {
    // Body text uses tx (near-black) for full readability; labels share the
    // same colour so the row doesn't fade. Icons use tx-2 (a half-step lighter)
    // to read as chrome rather than data.
    let body_font = egui::FontId::monospace(12.0); // text-xs = 12px
    let label_color = colors.state_outline;

    // Each icon is two-tone: chrome (tx-3 light gray) for the supporting frame,
    // accent (blue-400) for a single "one-point" element — the arrow shaft +
    // head for amplitude, the inner disk for probability, the arc for phase.
    let icon_accent = colors.popup_icon;
    let icon_chrome = colors.popup_icon_chrome;
    let labels = ["Amplitude:", "Probability:", "Phase:"];
    for (i, label) in labels.iter().enumerate() {
        let icon_rect = popup.icon_rect(i);
        match i {
            0 => draw_amplitude_icon(painter, icon_rect, icon_chrome, icon_accent),
            1 => draw_probability_icon(painter, icon_rect, icon_chrome, icon_accent),
            _ => draw_phase_icon(painter, icon_rect, icon_chrome, icon_accent),
        }
        painter.text(
            popup.label_pos(i),
            egui::Align2::LEFT_TOP,
            *label,
            body_font.clone(),
            label_color,
        );
    }
}
