use ab_glyph::{Font as _, ScaleFont as _};

const GLYPHS: &[u8] = b"0123456789%<>omitsgankep:";
const CELL_WIDTH: u32 = 10;
pub(super) const SURVIVAL_GLYPH_ATLAS_WIDTH: u32 = CELL_WIDTH * GLYPHS.len() as u32;
pub(super) const SURVIVAL_GLYPH_ATLAS_HEIGHT: u32 = crate::tailwind::LEADING_5 as u32;

pub(super) fn rasterize_survival_glyph_atlas() -> Vec<u8> {
    let font =
        ab_glyph::FontRef::try_from_slice(include_bytes!("../../assets/GeistMono-Regular.ttf"))
            .expect("Geist Mono Regular bytes should parse as a TTF");
    let scale = ab_glyph::PxScale::from(crate::tailwind::TEXT_SM * 1.2);
    let scaled = font.as_scaled(scale);
    let mut atlas = vec![0; (SURVIVAL_GLYPH_ATLAS_WIDTH * SURVIVAL_GLYPH_ATLAS_HEIGHT) as usize];
    for (i, ch) in GLYPHS.iter().enumerate() {
        let glyph = font.glyph_id(*ch as char).with_scale_and_position(
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
        let x = i as u32 * CELL_WIDTH + CELL_WIDTH.saturating_sub(bounds.width().ceil() as u32) / 2;
        let y = bounds.min.y.max(0.0) as u32;
        outlined.draw(|gx, gy, alpha| {
            let px = x + gx;
            let py = y + gy;
            if px < (i as u32 + 1) * CELL_WIDTH && py < SURVIVAL_GLYPH_ATLAS_HEIGHT {
                atlas[(py * SURVIVAL_GLYPH_ATLAS_WIDTH + px) as usize] = (alpha * 255.0) as u8;
            }
        });
    }
    atlas
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_survival_glyph_has_ink() {
        let atlas = rasterize_survival_glyph_atlas();
        assert!(
            (0..GLYPHS.len()).all(|i| (0..SURVIVAL_GLYPH_ATLAS_HEIGHT).any(|y| {
                (0..CELL_WIDTH).any(|x| {
                    atlas[(y * SURVIVAL_GLYPH_ATLAS_WIDTH + i as u32 * CELL_WIDTH + x) as usize]
                        != 0
                })
            }))
        );
    }
}
