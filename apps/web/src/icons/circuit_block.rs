//! Comment-block palette icon: the word "comment" set above a bold rule.

use eframe::egui;

use crate::colors::Colors;
use crate::tailwind::{SPACING_0_5, SPACING_1, TEXT_XS};

const WORD_TRACKING: f32 = -0.05 * TEXT_XS;

pub(crate) fn draw_circuit_block_palette_icon(
    painter: &egui::Painter,
    rect: egui::Rect,
    colors: &Colors,
) {
    let rule = egui::Rect::from_center_size(rect.center(), egui::vec2(rect.width(), SPACING_1));
    painter.rect_filled(rule, 0.0, colors.circuit_block_border);
    let word = painter.layout_job(egui::text::LayoutJob::single_section(
        "comment".to_owned(),
        egui::TextFormat {
            font_id: egui::FontId::proportional(TEXT_XS),
            extra_letter_spacing: WORD_TRACKING,
            color: colors.circuit_block_label,
            ..Default::default()
        },
    ));
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
    fn rule_is_centred_vertically() {
        assert_eq!(rule().center().y, ICON.center().y);
    }

    #[test]
    fn icon_writes_comment() {
        assert_eq!(word().0, "comment");
    }

    #[test]
    fn word_is_centred_horizontally() {
        assert!((word().1.center().x - ICON.center().x).abs() < 0.5);
    }

    #[test]
    fn word_sits_above_the_rule() {
        assert!(word().1.bottom() <= rule().top());
    }

    #[test]
    fn word_stays_inside_the_icon() {
        assert!(word().1.top() >= ICON.top());
    }
}
