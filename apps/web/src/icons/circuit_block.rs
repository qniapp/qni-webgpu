//! Comment-block palette icon: the word "block" set above a bold rule that
//! spans the full cell width, the pair centred as one group in the cell like
//! a placed block's label above its border.

use eframe::egui;

use crate::colors::Colors;
use crate::tailwind::{SPACING_0_5, SPACING_1, TEXT_SM};

pub(crate) fn draw_circuit_block_palette_icon(
    painter: &egui::Painter,
    rect: egui::Rect,
    colors: &Colors,
) {
    let word = painter.layout_job(egui::text::LayoutJob::single_section(
        "block".to_owned(),
        egui::TextFormat {
            font_id: egui::FontId::proportional(TEXT_SM),
            color: colors.circuit_block_label,
            ..Default::default()
        },
    ));
    let group_height = word.size().y + SPACING_0_5 + SPACING_1;
    let rule_top = (rect.center().y + group_height / 2.0 - SPACING_1).round();
    let rule = egui::Rect::from_min_size(
        egui::pos2(rect.left(), rule_top),
        egui::vec2(rect.width(), SPACING_1),
    );
    painter.rect_filled(rule, 0.0, colors.circuit_block_border);
    let word_rect = egui::Align2::CENTER_BOTTOM.anchor_size(
        egui::pos2(rect.center().x, rule.top() - SPACING_0_5),
        word.size(),
    );
    painter.galley(word_rect.min, word, colors.circuit_block_label);
}

#[cfg(test)]
mod tests {
    use super::*;

    const ICON: egui::Rect = egui::Rect {
        min: egui::pos2(100.0, 20.0),
        max: egui::pos2(140.0, 60.0),
    };

    fn icon_shapes() -> Vec<egui::Shape> {
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            draw_circuit_block_palette_icon(ui.painter(), ICON, &Colors::new());
        });
        output
            .shapes
            .into_iter()
            .map(|clipped| clipped.shape)
            .collect()
    }

    fn rule() -> egui::Rect {
        icon_shapes()
            .into_iter()
            .find_map(|shape| match shape {
                egui::Shape::Rect(rect) => Some(rect.rect),
                _ => None,
            })
            .expect("icon draws a rule")
    }

    fn word() -> (String, egui::Rect) {
        icon_shapes()
            .into_iter()
            .find_map(|shape| match shape {
                egui::Shape::Text(text) => Some((
                    text.galley.text().to_owned(),
                    text.galley.rect.translate(text.pos.to_vec2()),
                )),
                _ => None,
            })
            .expect("icon draws a word")
    }

    #[test]
    fn circuit_block_palette_icon_vector_snapshot() {
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            draw_circuit_block_palette_icon(ui.painter(), ICON, &Colors::new());
        });
        insta::assert_debug_snapshot!(output.shapes);
    }

    #[test]
    fn icon_draws_a_single_rule() {
        let rules = icon_shapes()
            .into_iter()
            .filter(|shape| matches!(shape, egui::Shape::Rect(_)))
            .count();
        assert_eq!(rules, 1);
    }

    #[test]
    fn rule_spans_the_full_icon_width() {
        let rule = rule();
        assert_eq!((rule.left(), rule.right()), (ICON.left(), ICON.right()));
    }

    #[test]
    fn rule_is_one_spacing_unit_thick() {
        assert_eq!(rule().height(), SPACING_1);
    }

    #[test]
    fn rule_sits_on_whole_pixels() {
        assert_eq!(rule().top().fract(), 0.0);
    }

    #[test]
    fn word_and_rule_are_centred_vertically_as_a_group() {
        let group_center = (word().1.top() + rule().bottom()) / 2.0;
        assert!((group_center - ICON.center().y).abs() <= 0.5);
    }

    #[test]
    fn word_is_set_in_text_sm() {
        let size = icon_shapes()
            .into_iter()
            .find_map(|shape| match shape {
                egui::Shape::Text(text) => text
                    .galley
                    .job
                    .sections
                    .first()
                    .map(|section| section.format.font_id.size),
                _ => None,
            })
            .expect("icon draws a word");
        assert_eq!(size, TEXT_SM);
    }

    #[test]
    fn icon_writes_block() {
        assert_eq!(word().0, "block");
    }

    #[test]
    fn word_is_narrower_than_the_rule() {
        assert!(word().1.width() < rule().width());
    }

    #[test]
    fn word_is_centred_horizontally() {
        assert!((word().1.center().x - ICON.center().x).abs() < 0.5);
    }

    #[test]
    fn word_sits_one_half_spacing_unit_above_the_rule() {
        assert!((rule().top() - word().1.bottom() - SPACING_0_5).abs() < 0.01);
    }

    #[test]
    fn word_stays_inside_the_icon() {
        assert!(word().1.top() >= ICON.top());
    }

    #[test]
    fn rule_stays_inside_the_icon() {
        assert!(rule().bottom() <= ICON.bottom());
    }
}
