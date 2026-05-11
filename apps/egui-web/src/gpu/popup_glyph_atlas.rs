//! Glyph atlas for the state-cell hover popup numeric values.
//!
//! The popup chrome (frame, icons, labels) is drawn by egui's painter
//! on the CPU side, but the amplitude / probability / phase *values*
//! live on the GPU (storage buffer holding the state vector). Per the
//! project's "no GPU→CPU readback in the render hot path" rule we
//! cannot ship those values back to the CPU just to render text — so a
//! shader pass formats and rasterises the numbers from a glyph atlas
//! directly inside the popup's value rects.
//!
//! This module bakes that atlas: a 1-row strip of fixed-width cells
//! holding the 16 glyphs needed to express any
//! ±d.ddddd ±d.ddddd i / ±dd.dddd % / ±ddd.dd ° formatted value. The
//! resulting greyscale buffer is uploaded to a GPU texture sampled by
//! the popup-value fragment shader.
//!
//! Sibling of `digit_atlas` (which handles the measurement-gate "0/1"
//! glyphs) — same approach, larger glyph set.
//!
//! Atlas layout (left → right):
//!   index 0..=9   : '0' .. '9'
//!   index 10      : '.'
//!   index 11      : '+'
//!   index 12      : '-'
//!   index 13      : 'i'  (imaginary suffix on amplitude)
//!   index 14      : '%'  (probability suffix)
//!   index 15      : '°'  (phase suffix)

use ab_glyph::{Font as _, ScaleFont as _};

/// Width of one glyph cell in atlas pixels. Sized to match Hack
/// monospace's advance width at `POPUP_GLYPH_PX_SCALE` (~8.6 px) plus a
/// thin safety margin — wide enough that no glyph bleeds into the
/// neighbouring cell, narrow enough that the rendered text matches
/// egui's own monospace label spacing on screen. Exposed crate-wide so
/// the CPU-side popup layout can use the exact same cell size the
/// shader samples from.
pub(crate) const POPUP_GLYPH_CELL_W: u32 = 9;
/// Height of one glyph cell in atlas pixels.
pub(crate) const POPUP_GLYPH_CELL_H: u32 = 16;
/// Number of cells in the atlas strip. Matches the glyph set below.
pub(super) const POPUP_GLYPH_COUNT: u32 = 16;
pub(super) const POPUP_GLYPH_ATLAS_WIDTH: u32 = POPUP_GLYPH_CELL_W * POPUP_GLYPH_COUNT;
pub(super) const POPUP_GLYPH_ATLAS_HEIGHT: u32 = POPUP_GLYPH_CELL_H;

/// Glyphs in the order they appear in the atlas strip.
pub(super) const POPUP_GLYPHS: [char; 16] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '.', '+', '-', 'i', '%', '°',
];

/// PxScale calibrated so the rasterised glyph height matches what egui
/// renders for `FontId::monospace(12.0)`. The factor of ~1.2 mirrors
/// the calibration in `digit_atlas`: egui internally upscales mono past
/// the raw em-size that `ab_glyph::PxScale::from(12.0)` would give.
const POPUP_GLYPH_PX_SCALE: f32 = 14.4;

/// Rasterise the popup-value glyphs into a single-channel (alpha) byte
/// buffer of size `POPUP_GLYPH_ATLAS_WIDTH × POPUP_GLYPH_ATLAS_HEIGHT`.
/// Each glyph is horizontally centred in its cell and baseline-aligned
/// against the font's ascent (so digits, `+`, `-`, `.`, `i`, `%`, `°`
/// all sit on a shared baseline). Used at startup; the result is
/// uploaded once to a GPU texture.
pub(super) fn rasterize_popup_glyph_atlas() -> Vec<u8> {
    let font = ab_glyph::FontRef::try_from_slice(epaint_default_fonts::HACK_REGULAR)
        .expect("Hack Regular bytes should parse as a TTF");
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
        // Vertical origin: align glyphs by their px_bounds top (≈ font
        // ascender). For visually consistent baseline-alignment we offset
        // by the glyph's bounds.min.y so that the glyph's top-left lands
        // at the cell's top edge plus the glyph's natural ascent.
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

    /// Atlas dimensions are derived from `POPUP_GLYPH_COUNT` * `CELL_W`
    /// — guard against the buffer length getting out of sync with
    /// either constant.
    #[test]
    fn atlas_buffer_length_matches_dimensions() {
        let atlas = rasterize_popup_glyph_atlas();
        assert_eq!(
            atlas.len() as u32,
            POPUP_GLYPH_ATLAS_WIDTH * POPUP_GLYPH_ATLAS_HEIGHT
        );
    }

    /// Each glyph cell must contain non-zero pixels — if any glyph
    /// failed to rasterise (missing in the font, off-cell, etc.) the
    /// popup text would silently render as a blank.
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

    /// Adjacent glyph cells must not bleed into one another: the
    /// single-column gap between cells (last pixel of cell N, first
    /// pixel of cell N+1) is reserved space and would cause UV
    /// aliasing in the shader if a glyph spilled into it. We assert
    /// the edge columns are dark for every cell boundary.
    #[test]
    fn cells_do_not_overlap_horizontally() {
        let atlas = rasterize_popup_glyph_atlas();
        for boundary in 1..POPUP_GLYPH_COUNT {
            let last_col_of_prev = boundary * POPUP_GLYPH_CELL_W - 1;
            let first_col_of_next = boundary * POPUP_GLYPH_CELL_W;
            for y in 0..POPUP_GLYPH_ATLAS_HEIGHT {
                let prev_px = atlas[(y * POPUP_GLYPH_ATLAS_WIDTH + last_col_of_prev) as usize];
                let next_px = atlas[(y * POPUP_GLYPH_ATLAS_WIDTH + first_col_of_next) as usize];
                // Allow a touch of antialiasing fringe (≤ 32/255 ≈
                // 12 %), but anything brighter means a glyph spilled
                // past the cell.
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
