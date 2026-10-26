struct ArithmeticParams {
    state_count: u32,
    target_shift: u32,
    target_mask: u32,
    amount: u32,
    control_mask: u32,
    control_value: u32,
    _pad0: u32,
    _pad1: u32,
};
@group(0) @binding(0) var<storage, read> state_in: array<vec2<f32>>;
@group(0) @binding(1) var<storage, read_write> state_out: array<vec2<f32>>;
@group(0) @binding(2) var<uniform> params: ArithmeticParams;

// GENERATED_ARITHMETIC_INDEX

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= params.state_count) { return; }
    let source = source_index(i, params.target_shift, params.target_mask,
        params.amount, params.control_mask, params.control_value);
    state_out[i] = state_in[source];
}
