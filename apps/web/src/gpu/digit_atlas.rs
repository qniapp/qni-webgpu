//! SDF atlas for the measurement-digit overlay.
//!
//! The digit source is shared with the Write gate digits:
//! `assets/icons/digit0.svg` and `assets/icons/digit1.svg` are rasterised by
//! `build.rs`, converted to SDF, and packed here into a 1×2 atlas sampled by
//! `MEASUREMENT_DIGIT_SHADER`.

use crate::icons::{digit_sdf_rle, DIGIT_SDF_PX_RANGE, DIGIT_SDF_SIZE};

/// Atlas geometry for the measurement digit texture: a 1×2 grid of full
/// SVG-viewBox SDF cells. The shader draws each cell into a 32 px quad so
/// Measurement digits match the Write gate digits exactly.
pub(super) const DIGIT_ATLAS_CELL: u32 = DIGIT_SDF_SIZE as u32;
pub(super) const DIGIT_ATLAS_WIDTH: u32 = DIGIT_ATLAS_CELL;
pub(super) const DIGIT_ATLAS_HEIGHT: u32 = DIGIT_ATLAS_CELL * 2;
pub(super) const DIGIT_ATLAS_SDF_PX_RANGE: f32 = DIGIT_SDF_PX_RANGE;

pub(super) fn rasterize_digit_sdf_atlas() -> Vec<u8> {
    let cell_len = DIGIT_SDF_SIZE * DIGIT_SDF_SIZE;
    let mut atlas = Vec::with_capacity(cell_len * 2);
    append_rle(&mut atlas, digit_sdf_rle(0));
    append_rle(&mut atlas, digit_sdf_rle(1));
    debug_assert_eq!(atlas.len(), cell_len * 2);
    atlas
}

fn append_rle(out: &mut Vec<u8>, rle: &[(u16, u8)]) {
    for &(run, value) in rle {
        out.extend(std::iter::repeat_n(value, run as usize));
    }
}
