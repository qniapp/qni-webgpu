// Shared only with host verification and build-time WGSL generation.
// Controls are external to the contiguous target field; shifts must be < 32.
pub(crate) fn source_index(
    i: u32,
    target_shift: u32,
    target_mask: u32,
    amount: u32,
    control_mask: u32,
    control_value: u32,
) -> u32 {
    if (i & control_mask) != control_value {
        return i;
    }
    let field = (i >> target_shift) & target_mask;
    let previous = field.wrapping_sub(amount) & target_mask;
    let mask = target_mask << target_shift;
    (i & !mask) | (previous << target_shift)
}
