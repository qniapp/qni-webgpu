//! Glyph rasterization for the measurement-digit overlay.
//!
//! Done once at startup; the resulting greyscale buffer is uploaded to a
//! GPU texture sampled by `MEASUREMENT_DIGIT_SHADER` (see `shaders.rs`).
//! Lives outside `resources.rs` so the heavy `ab_glyph` import stays
//! contained.

use ab_glyph::{Font as _, ScaleFont as _};

/// Atlas geometry for the measurement digit texture: a 1x2 grid of cells,
/// each holding a single rasterised glyph. Cell size matches the
/// `MeasurementDigitInstance::half_extent * 2` quad the shader draws into,
/// so UVs map identity-style.
pub(super) const DIGIT_ATLAS_CELL: u32 = 22;
pub(super) const DIGIT_ATLAS_WIDTH: u32 = DIGIT_ATLAS_CELL;
pub(super) const DIGIT_ATLAS_HEIGHT: u32 = DIGIT_ATLAS_CELL * 2;

/// Rasterises the digits "0" and "1" using Hack Regular (the same font
/// egui's monospace family resolves to) so the measurement digits look
/// identical to the |0> / |1> labels egui paints. Done once at startup;
/// the result is uploaded to a GPU texture sampled by
/// `MEASUREMENT_DIGIT_SHADER`. The PxScale is calibrated so the rasterised
/// "0" matches the on-screen size of `FontId::monospace(16.0)`'s glyph
/// (egui internally upscales monospace ~1.2x past the raw em-size we'd
/// get from a plain ab_glyph rasterisation at PxScale(16)).
pub(super) fn rasterize_digit_atlas() -> Vec<u8> {
    let font = ab_glyph::FontRef::try_from_slice(epaint_default_fonts::HACK_REGULAR)
        .expect("Hack Regular bytes should parse as a TTF");
    let scale = ab_glyph::PxScale::from(20.0);
    let scaled = font.as_scaled(scale);

    let mut atlas = vec![0u8; (DIGIT_ATLAS_WIDTH * DIGIT_ATLAS_HEIGHT) as usize];
    for (cell_index, ch) in ['0', '1'].iter().enumerate() {
        let glyph_id = font.glyph_id(*ch);
        let glyph =
            glyph_id.with_scale_and_position(scale, ab_glyph::Point { x: 0.0, y: scaled.ascent() });
        let Some(outlined) = font.outline_glyph(glyph) else {
            continue;
        };
        let bounds = outlined.px_bounds();
        let glyph_w = bounds.width().ceil() as u32;
        let glyph_h = bounds.height().ceil() as u32;
        let cell_origin_x = DIGIT_ATLAS_CELL.saturating_sub(glyph_w) / 2;
        let cell_origin_y =
            cell_index as u32 * DIGIT_ATLAS_CELL + DIGIT_ATLAS_CELL.saturating_sub(glyph_h) / 2;
        outlined.draw(|gx, gy, alpha| {
            let px = cell_origin_x + gx;
            let py = cell_origin_y + gy;
            if px < DIGIT_ATLAS_WIDTH && py < DIGIT_ATLAS_HEIGHT {
                atlas[(py * DIGIT_ATLAS_WIDTH + px) as usize] =
                    (alpha.clamp(0.0, 1.0) * 255.0) as u8;
            }
        });
    }
    atlas
}
