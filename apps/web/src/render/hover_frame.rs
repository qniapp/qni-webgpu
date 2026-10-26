use eframe::egui;

use crate::gates::GateKind;

const ROUNDED_GATE_HOVER_FRAME_RADIUS: u8 = 10;
const ROUNDED_GATE_HOVER_GAP_RADIUS: u8 = 8;

fn display_block_has_square_body(kind: GateKind) -> bool {
    matches!(
        kind,
        GateKind::ProbabilityDisplay | GateKind::AmplitudeDisplay | GateKind::DensityMatrixDisplay
    )
}

pub(crate) fn hover_frame_corner_radius(kind: GateKind) -> egui::CornerRadius {
    if display_block_has_square_body(kind) {
        egui::CornerRadius::ZERO
    } else {
        egui::CornerRadius::same(ROUNDED_GATE_HOVER_FRAME_RADIUS)
    }
}

/// Palette-only inner cutout corner. It must stay square for square display
/// blocks; otherwise the filled gap would reintroduce rounded corners.
pub(crate) fn hover_frame_inner_corner_radius(kind: GateKind) -> egui::CornerRadius {
    if display_block_has_square_body(kind) {
        egui::CornerRadius::ZERO
    } else {
        egui::CornerRadius::same(ROUNDED_GATE_HOVER_GAP_RADIUS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_hover_frame_keeps_rounded_tile_corners() {
        assert_eq!(
            hover_frame_corner_radius(GateKind::SampleDisplay),
            egui::CornerRadius::same(ROUNDED_GATE_HOVER_FRAME_RADIUS)
        );
    }
}
