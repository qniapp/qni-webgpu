use ab_glyph::{Font as _, ScaleFont as _};

pub(crate) const POPUP_GLYPH_CELL_W: u32 = 9;
pub(crate) const POPUP_GLYPH_CELL_H: u32 = 16;
pub(super) const POPUP_GLYPH_COUNT: u32 = 20;
pub(super) const POPUP_GLYPH_ATLAS_WIDTH: u32 = POPUP_GLYPH_CELL_W * POPUP_GLYPH_COUNT;
pub(super) const POPUP_GLYPH_ATLAS_HEIGHT: u32 = POPUP_GLYPH_CELL_H;

pub(super) const POPUP_GLYPHS: [char; 20] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '.', '+', '−', 'i', '%', '°', 'd', 'B', '∞',
    '—',
];

const POPUP_GLYPH_PX_SCALE: f32 = 14.4;

pub(super) fn rasterize_popup_glyph_atlas() -> Vec<u8> {
    let font =
        ab_glyph::FontRef::try_from_slice(include_bytes!("../../assets/GeistMono-Regular.ttf"))
            .expect("Geist Mono Regular bytes should parse as a TTF");
    let scale = ab_glyph::PxScale::from(POPUP_GLYPH_PX_SCALE);
    let scaled = font.as_scaled(scale);

    let mut atlas = vec![0u8; (POPUP_GLYPH_ATLAS_WIDTH * POPUP_GLYPH_ATLAS_HEIGHT) as usize];
    for (cell_index, ch) in POPUP_GLYPHS.iter().enumerate() {
        let glyph_id = font.glyph_id(*ch);
        let glyph = glyph_id.with_scale_and_position(
            scale,
            ab_glyph::Point {
                x: 0.0,
                y: scaled.ascent(),
            },
        );
        let Some(outlined) = font.outline_glyph(glyph) else {
            continue;
        };
        let bounds = outlined.px_bounds();
        let glyph_w = bounds.width().ceil() as u32;
        let cell_origin_x =
            cell_index as u32 * POPUP_GLYPH_CELL_W + POPUP_GLYPH_CELL_W.saturating_sub(glyph_w) / 2;
        let cell_origin_y = bounds.min.y.max(0.0) as u32;
        outlined.draw(|gx, gy, alpha| {
            let px = cell_origin_x + gx;
            let py = cell_origin_y + gy;
            if px < POPUP_GLYPH_ATLAS_WIDTH && py < POPUP_GLYPH_ATLAS_HEIGHT {
                atlas[(py * POPUP_GLYPH_ATLAS_WIDTH + px) as usize] =
                    (alpha.clamp(0.0, 1.0) * 255.0) as u8;
            }
        });
    }
    atlas
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_glyph_counts_match_atlas() {
        let declaration = format!("const GLYPH_COUNT: u32 = {POPUP_GLYPH_COUNT}u;");
        let shaders = [
            include_str!("shaders/popup_value.wgsl"),
            include_str!("shaders/bloch_popup_value.wgsl"),
            include_str!("shaders/amplitude_popup_value.wgsl"),
            include_str!("shaders/probability_popup_value.wgsl"),
            include_str!("shaders/probability_render.wgsl"),
        ];
        assert!(shaders.iter().all(|shader| shader.contains(&declaration)));
    }

    #[test]
    fn atlas_buffer_length_matches_dimensions() {
        let atlas = rasterize_popup_glyph_atlas();
        assert_eq!(
            atlas.len() as u32,
            POPUP_GLYPH_ATLAS_WIDTH * POPUP_GLYPH_ATLAS_HEIGHT
        );
    }

    #[test]
    fn every_glyph_cell_has_ink() {
        let atlas = rasterize_popup_glyph_atlas();
        for (i, ch) in POPUP_GLYPHS.iter().enumerate() {
            let cell_x0 = i as u32 * POPUP_GLYPH_CELL_W;
            let mut nonzero_pixels = 0u32;
            for y in 0..POPUP_GLYPH_ATLAS_HEIGHT {
                for x in cell_x0..(cell_x0 + POPUP_GLYPH_CELL_W) {
                    if atlas[(y * POPUP_GLYPH_ATLAS_WIDTH + x) as usize] > 0 {
                        nonzero_pixels += 1;
                    }
                }
            }
            assert!(
                nonzero_pixels > 0,
                "glyph #{i} ('{ch}') rasterised to all zeroes — \
                 font may be missing the codepoint or PxScale is wrong"
            );
        }
    }

    #[test]
    fn cells_do_not_overlap_horizontally() {
        let atlas = rasterize_popup_glyph_atlas();
        for boundary in 1..POPUP_GLYPH_COUNT {
            let last_col_of_prev = boundary * POPUP_GLYPH_CELL_W - 1;
            let first_col_of_next = boundary * POPUP_GLYPH_CELL_W;
            for y in 0..POPUP_GLYPH_ATLAS_HEIGHT {
                let prev_px = atlas[(y * POPUP_GLYPH_ATLAS_WIDTH + last_col_of_prev) as usize];
                let next_px = atlas[(y * POPUP_GLYPH_ATLAS_WIDTH + first_col_of_next) as usize];
                assert!(
                    prev_px < 32 && next_px < 32,
                    "glyph cell {} bleeds into cell {} at y={y}: \
                     prev_px={prev_px}, next_px={next_px}",
                    boundary - 1,
                    boundary,
                );
            }
        }
    }
}
