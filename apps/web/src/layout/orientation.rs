use eframe::egui;

use crate::tailwind::SCREEN_SM;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum CircuitOrientation {
    #[default]
    Horizontal,
    Vertical,
}

impl CircuitOrientation {
    pub(crate) fn for_canvas(canvas: egui::Vec2) -> Self {
        if canvas.x < SCREEN_SM && canvas.y >= canvas.x {
            Self::Vertical
        } else {
            Self::Horizontal
        }
    }

    pub(crate) fn is_vertical(self) -> bool {
        self == Self::Vertical
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn orientation(width: f32, height: f32) -> CircuitOrientation {
        CircuitOrientation::for_canvas(egui::vec2(width, height))
    }

    #[test]
    fn phone_portrait_390x844_is_vertical() {
        assert_eq!(orientation(390.0, 844.0), CircuitOrientation::Vertical);
    }

    #[test]
    fn phone_landscape_844x390_is_horizontal() {
        assert_eq!(orientation(844.0, 390.0), CircuitOrientation::Horizontal);
    }

    #[test]
    fn tablet_portrait_768x1024_is_horizontal() {
        assert_eq!(orientation(768.0, 1024.0), CircuitOrientation::Horizontal);
    }

    #[test]
    fn width_640_portrait_is_horizontal() {
        assert_eq!(orientation(640.0, 900.0), CircuitOrientation::Horizontal);
    }

    #[test]
    fn width_639_portrait_is_vertical() {
        assert_eq!(orientation(639.0, 900.0), CircuitOrientation::Vertical);
    }

    #[test]
    fn narrow_square_canvas_is_vertical() {
        assert_eq!(orientation(500.0, 500.0), CircuitOrientation::Vertical);
    }

    #[test]
    fn narrow_landscape_window_500x300_is_horizontal() {
        assert_eq!(orientation(500.0, 300.0), CircuitOrientation::Horizontal);
    }

    #[test]
    fn desktop_1000x800_is_horizontal() {
        assert_eq!(orientation(1000.0, 800.0), CircuitOrientation::Horizontal);
    }
}
