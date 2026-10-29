use crate::icons::{digit_sdf_rle, DIGIT_SDF_PX_RANGE, DIGIT_SDF_SIZE};

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
