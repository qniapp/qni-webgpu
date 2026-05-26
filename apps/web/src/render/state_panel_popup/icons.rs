use eframe::egui;

/// Amplitude icon — Re/Im axes with the origin pushed into the
/// lower-left corner so the first quadrant fills ~75% of the
/// frame. That leaves room for a large diagonal arrow representing
/// the complex amplitude. Origin at (3, 13); axes stay thin so the
/// arrow reads as foreground. Arrow shaft 2.2 px, filled triangular
/// head.
pub(in crate::render) fn draw_amplitude_icon(
    painter: &egui::Painter,
    rect: egui::Rect,
    chrome: egui::Color32,
    accent: egui::Color32,
) {
    let s = rect.width() / 16.0;
    let axis_stroke = egui::Stroke::new(1.2 * s, chrome);
    let shaft = egui::Stroke::new(2.2 * s, accent);
    let origin = egui::pos2(rect.min.x + 3.0 * s, rect.min.y + 13.0 * s);
    // Re axis (horizontal at y = 13). A tiny stub on the left
    // (x = 1..3) hints at the negative region; the bulk (x = 3..15)
    // is the +Re half-line.
    painter.line_segment(
        [
            egui::pos2(rect.min.x + 1.0 * s, origin.y),
            egui::pos2(rect.min.x + 15.0 * s, origin.y),
        ],
        axis_stroke,
    );
    // Im axis (vertical at x = 3). Stub below (y = 13..15) and the
    // bulk (y = 1..13) is +Im.
    painter.line_segment(
        [
            egui::pos2(origin.x, rect.min.y + 1.0 * s),
            egui::pos2(origin.x, rect.min.y + 15.0 * s),
        ],
        axis_stroke,
    );
    // Origin dot — chrome, supporting role.
    painter.circle_filled(origin, 1.4 * s, chrome);
    // Arrow shaft + head — blue accent. Tip near (14, 2).
    let tip = egui::pos2(rect.min.x + 14.0 * s, rect.min.y + 2.0 * s);
    let dir = (tip - origin).normalized();
    // Stop the shaft well short of the tip so the head reads as a
    // distinct triangle, not a thick stub on the shaft.
    let head_len = 5.2 * s;
    let head_half = 3.4 * s;
    let shaft_end = tip - dir * (head_len - 0.4 * s);
    painter.line_segment([origin, shaft_end], shaft);
    // Arrowhead — filled triangle in accent. perp is 90° from dir.
    let perp = egui::vec2(dir.y, -dir.x);
    let base_centre = tip - dir * head_len;
    let base_l = base_centre + perp * head_half;
    let base_r = base_centre - perp * head_half;
    painter.add(egui::Shape::convex_polygon(
        vec![tip, base_l, base_r],
        accent,
        egui::Stroke::NONE,
    ));
}

/// Probability icon — chrome outer ring + accent (blue) inner disk.
/// The blue inner disk reads as "the value" (probability mass);
/// the gray ring frames it without competing.
pub(in crate::render) fn draw_probability_icon(
    painter: &egui::Painter,
    rect: egui::Rect,
    chrome: egui::Color32,
    accent: egui::Color32,
) {
    let s = rect.width() / 16.0;
    let centre = egui::pos2(rect.min.x + 8.0 * s, rect.min.y + 8.0 * s);
    painter.circle_stroke(centre, 6.4 * s, egui::Stroke::new(2.0 * s, chrome));
    painter.circle_filled(centre, 4.0 * s, accent);
}

/// Phase icon — chrome base line + hypotenuse with an accent (blue)
/// arc near the origin marking the swept angle. The arc is the
/// "the value" (phase angle); the two straight lines just frame the
/// triangle.
pub(in crate::render) fn draw_phase_icon(
    painter: &egui::Painter,
    rect: egui::Rect,
    chrome: egui::Color32,
    accent: egui::Color32,
) {
    let s = rect.width() / 16.0;
    let chrome_stroke = egui::Stroke::new(1.6 * s, chrome);
    let accent_stroke = egui::Stroke::new(2.0 * s, accent);
    let base_l = egui::pos2(rect.min.x + 2.5 * s, rect.min.y + 12.6 * s);
    let base_r = egui::pos2(rect.min.x + 14.2 * s, rect.min.y + 12.6 * s);
    let hyp_top = egui::pos2(rect.min.x + 10.23 * s, rect.min.y + 3.09 * s);
    painter.line_segment([base_l, base_r], chrome_stroke);
    painter.line_segment([base_l, hyp_top], chrome_stroke);
    // Arc near the origin — approximated as a short polyline, drawn
    // in the blue accent so the "phase angle" reads as the subject.
    let arc_pts = [
        egui::pos2(rect.min.x + 6.02 * s, rect.min.y + 8.95 * s),
        egui::pos2(rect.min.x + 7.10 * s, rect.min.y + 10.60 * s),
        egui::pos2(rect.min.x + 7.90 * s, rect.min.y + 12.95 * s),
    ];
    painter.line_segment([arc_pts[0], arc_pts[1]], accent_stroke);
    painter.line_segment([arc_pts[1], arc_pts[2]], accent_stroke);
}
