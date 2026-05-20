use eframe::egui;
use std::sync::Arc;

use super::layout::{PAD_X, PAD_Y};
use crate::colors::Colors;
use crate::gates::GateInfo;

pub(super) const TITLE_GAP: f32 = 4.0; // mt-1 between title and first paragraph
pub(super) const PARA_GAP: f32 = 2.0; // mt-0.5 between paragraphs
pub(super) const DIAGRAM_GAP: f32 = 16.0; // .tooltip-body { mt-4 }

pub(super) struct TooltipText {
    title: Arc<egui::Galley>,
    paragraphs: Vec<Arc<egui::Galley>>,
}

impl TooltipText {
    pub(super) fn content_size(&self, diagram_size: egui::Vec2) -> egui::Vec2 {
        let desc_block_h = self.description_height();
        let desc_w = self.description_width();
        let content_w = self.title.size().x.max(desc_w).max(diagram_size.x);
        let mut content_h = self.title.size().y;
        if desc_block_h > 0.0 {
            content_h += TITLE_GAP + desc_block_h;
        }
        if diagram_size.y > 0.0 {
            content_h += DIAGRAM_GAP + diagram_size.y;
        }
        egui::vec2(content_w, content_h)
    }

    fn description_height(&self) -> f32 {
        if self.paragraphs.is_empty() {
            0.0
        } else {
            self.paragraphs.iter().map(|g| g.size().y).sum::<f32>()
                + PARA_GAP * (self.paragraphs.len() as f32 - 1.0)
        }
    }

    fn description_width(&self) -> f32 {
        self.paragraphs
            .iter()
            .map(|g| g.size().x)
            .fold(0.0_f32, f32::max)
    }
}

pub(super) fn layout_tooltip_text(
    painter: &egui::Painter,
    info: &GateInfo,
    colors: &Colors,
) -> TooltipText {
    // Text layout sizes mirror qni's `.tooltip-*` utilities:
    // title = `text-lg` (18 px) `font-bold` tx — qni's
    // `.tooltip-heading`. We can't render true bold without bundling a
    // bold font; rely on size + colour contrast for hierarchy.
    // para = `text-sm` (14 px) tx-2 — `.tooltip-subheading`.
    let title = painter.layout_no_wrap(
        info.name.to_owned(),
        egui::FontId::proportional(18.0),
        colors.text_strong,
    );
    let paragraphs = info
        .paragraphs
        .iter()
        .map(|line| {
            painter.layout_no_wrap(
                (*line).to_owned(),
                egui::FontId::proportional(14.0),
                colors.text,
            )
        })
        .collect();

    TooltipText { title, paragraphs }
}

pub(super) fn paint_tooltip_text(
    painter: &egui::Painter,
    card_rect: egui::Rect,
    text: &TooltipText,
    colors: &Colors,
) -> f32 {
    let title_pos = card_rect.min + egui::vec2(PAD_X, PAD_Y);
    let title_h = text.title.size().y;
    painter.galley(title_pos, text.title.clone(), colors.text_strong);

    let mut cursor_y = title_pos.y + title_h;
    if !text.paragraphs.is_empty() {
        cursor_y += TITLE_GAP;
    }
    for galley in &text.paragraphs {
        let h = galley.size().y;
        painter.galley(
            egui::pos2(title_pos.x, cursor_y),
            galley.clone(),
            colors.text,
        );
        cursor_y += h + PARA_GAP;
    }

    cursor_y
}
