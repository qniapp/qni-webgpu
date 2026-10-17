use eframe::egui;

use super::CircuitOrientation;

/// Maps circuit space (x = time, y = wire) to the screen. Horizontal is a
/// translation; vertical is the same translation after a 90° clockwise
/// rotation, so time runs down and later wires sit further left. Text and
/// gate glyphs are never rotated: callers project anchor points and rects
/// and draw upright inside them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CircuitProjection {
    orientation: CircuitOrientation,
    origin: egui::Pos2,
}

impl CircuitProjection {
    /// `origin` is the screen position of the circuit-space origin.
    pub(crate) fn new(orientation: CircuitOrientation, origin: egui::Pos2) -> Self {
        Self {
            orientation,
            origin,
        }
    }

    pub(crate) fn horizontal(origin: egui::Pos2) -> Self {
        Self::new(CircuitOrientation::Horizontal, origin)
    }

    pub(crate) fn is_vertical(self) -> bool {
        self.orientation.is_vertical()
    }

    pub(crate) fn to_screen_vec(self, local: egui::Vec2) -> egui::Vec2 {
        match self.orientation {
            CircuitOrientation::Horizontal => local,
            CircuitOrientation::Vertical => egui::vec2(-local.y, local.x),
        }
    }

    pub(crate) fn to_screen(self, local: egui::Pos2) -> egui::Pos2 {
        self.origin + self.to_screen_vec(local.to_vec2())
    }

    pub(crate) fn to_local(self, screen: egui::Pos2) -> egui::Pos2 {
        let delta = screen - self.origin;
        match self.orientation {
            CircuitOrientation::Horizontal => delta.to_pos2(),
            CircuitOrientation::Vertical => egui::pos2(delta.y, -delta.x),
        }
    }

    /// Screen rect covering a circuit-space rect. Width and height swap in
    /// the vertical layout.
    pub(crate) fn to_screen_rect(self, local: egui::Rect) -> egui::Rect {
        match self.orientation {
            CircuitOrientation::Horizontal => local.translate(self.origin.to_vec2()),
            CircuitOrientation::Vertical => {
                self.to_screen_rect_from_min_size(local.min, local.size())
            }
        }
    }

    /// `to_screen_rect` of the circuit-space rect at `min` with `size`.
    pub(crate) fn to_screen_rect_from_min_size(
        self,
        min: egui::Pos2,
        size: egui::Vec2,
    ) -> egui::Rect {
        match self.orientation {
            CircuitOrientation::Horizontal => egui::Rect::from_min_size(self.to_screen(min), size),
            CircuitOrientation::Vertical => {
                egui::Rect::from_two_pos(self.to_screen(min), self.to_screen(min + size))
            }
        }
    }

    /// Screen direction in which time advances.
    pub(crate) fn time_axis(self) -> egui::Vec2 {
        self.to_screen_vec(egui::Vec2::X)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{GATE_SIZE, LINE_GAP, LINE_Y, SLOT_SPACING};

    const ORIGIN: egui::Pos2 = egui::pos2(437.0, -61.5);

    fn grid_points() -> Vec<egui::Pos2> {
        (0..=20)
            .flat_map(|column| {
                (0..=16).map(move |wire| {
                    egui::pos2(
                        column as f32 * SLOT_SPACING * 0.5,
                        LINE_Y + wire as f32 * LINE_GAP * 0.5,
                    )
                })
            })
            .collect()
    }

    fn roundtrip(projection: CircuitProjection) -> Vec<egui::Pos2> {
        grid_points()
            .into_iter()
            .map(|point| projection.to_local(projection.to_screen(point)))
            .collect()
    }

    fn vertical() -> CircuitProjection {
        CircuitProjection::new(CircuitOrientation::Vertical, ORIGIN)
    }

    #[test]
    fn vertical_to_local_inverts_to_screen() {
        assert_eq!(roundtrip(vertical()), grid_points());
    }

    #[test]
    fn horizontal_to_local_inverts_to_screen() {
        assert_eq!(
            roundtrip(CircuitProjection::horizontal(ORIGIN)),
            grid_points()
        );
    }

    #[test]
    fn vertical_screen_to_local_inverts_to_local() {
        let screens = (0..20)
            .flat_map(|x| (0..20).map(move |y| egui::pos2(x as f32 * 19.5, y as f32 * 42.0)))
            .collect::<Vec<_>>();
        let projection = vertical();

        assert_eq!(
            screens
                .iter()
                .map(|&screen| projection.to_screen(projection.to_local(screen)))
                .collect::<Vec<_>>(),
            screens
        );
    }

    #[test]
    fn horizontal_projection_matches_former_origin_translation() {
        let projection = CircuitProjection::horizontal(ORIGIN);

        assert_eq!(
            grid_points()
                .into_iter()
                .map(|point| projection.to_screen(point))
                .collect::<Vec<_>>(),
            grid_points()
                .into_iter()
                .map(|point| ORIGIN + point.to_vec2())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn horizontal_rect_matches_former_translation() {
        let local = egui::Rect::from_min_size(egui::pos2(142.3, 236.7), egui::vec2(96.1, 152.9));

        assert_eq!(
            CircuitProjection::horizontal(ORIGIN).to_screen_rect(local),
            local.translate(ORIGIN.to_vec2())
        );
    }

    #[test]
    fn horizontal_gate_rect_matches_former_min_size_rect() {
        let min = egui::pos2(142.3, 236.7);
        let size = egui::vec2(40.0, 152.0);

        assert_eq!(
            CircuitProjection::horizontal(ORIGIN).to_screen_rect_from_min_size(min, size),
            egui::Rect::from_min_size(ORIGIN + min.to_vec2(), size)
        );
    }

    #[test]
    fn vertical_rect_roundtrip_preserves_local_rect() {
        let projection = vertical();
        let local = egui::Rect::from_min_size(egui::pos2(142.0, 236.0), egui::vec2(96.0, 152.0));
        let screen = projection.to_screen_rect(local);

        assert_eq!(
            egui::Rect::from_two_pos(
                projection.to_local(screen.min),
                projection.to_local(screen.max)
            ),
            local
        );
    }

    #[test]
    fn vertical_wire0_is_right_of_wire1() {
        let projection = vertical();
        let wire0 = projection.to_screen(egui::pos2(0.0, LINE_Y));
        let wire1 = projection.to_screen(egui::pos2(0.0, LINE_Y + LINE_GAP));

        assert!(wire0.x > wire1.x);
    }

    #[test]
    fn vertical_column1_is_below_column0() {
        let projection = vertical();
        let column0 = projection.to_screen(egui::pos2(0.0, LINE_Y));
        let column1 = projection.to_screen(egui::pos2(SLOT_SPACING, LINE_Y));

        assert!(column1.y > column0.y);
    }

    #[test]
    fn vertical_span3_probability_rect_is_wide() {
        let local = egui::Rect::from_min_size(
            egui::pos2(0.0, LINE_Y),
            egui::vec2(GATE_SIZE, GATE_SIZE + 2.0 * LINE_GAP),
        );

        assert_eq!(
            vertical().to_screen_rect(local).size(),
            egui::vec2(GATE_SIZE + 2.0 * LINE_GAP, GATE_SIZE)
        );
    }

    #[test]
    fn vertical_projection_is_rotation_not_mirror() {
        let projection = vertical();

        assert_eq!(
            projection.time_axis().x * projection.to_screen_vec(egui::Vec2::Y).y
                - projection.time_axis().y * projection.to_screen_vec(egui::Vec2::Y).x,
            1.0
        );
    }

    #[test]
    fn vertical_time_runs_down() {
        assert_eq!(vertical().time_axis(), egui::Vec2::Y);
    }

    #[test]
    fn vertical_wires_advance_left() {
        assert_eq!(vertical().to_screen_vec(egui::Vec2::Y), -egui::Vec2::X);
    }
}
