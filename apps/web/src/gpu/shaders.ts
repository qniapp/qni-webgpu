import { STATE_TEXT_MAX_LEN } from '../ui/constants'

export const computeShaderCode = `
struct GateParams {
  gateType: u32,
  targetIndex: u32,
  _pad1: u32,
  _pad2: u32,
}

@group(0) @binding(0) var<uniform> params: GateParams;
@group(0) @binding(1) var<storage, read> inputState: array<vec2<f32>, 4>;
@group(0) @binding(2) var<storage, read_write> outputState: array<vec2<f32>, 4>;

fn cadd(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
  return a + b;
}

fn csub(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
  return a - b;
}

fn cmul(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
  return vec2<f32>(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}

fn gate_matrix(gateType: u32) -> array<vec2<f32>, 4> {
  let invSqrt2 = 0.7071067811865476;
  switch gateType {
    case 0u: { // H
      return array<vec2<f32>, 4>(
        vec2<f32>(invSqrt2, 0.0),
        vec2<f32>(invSqrt2, 0.0),
        vec2<f32>(invSqrt2, 0.0),
        vec2<f32>(-invSqrt2, 0.0)
      );
    }
    case 1u: { // X
      return array<vec2<f32>, 4>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 0.0)
      );
    }
    case 2u: { // Y
      return array<vec2<f32>, 4>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, -1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 0.0)
      );
    }
    case 3u: { // Z
      return array<vec2<f32>, 4>(
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(-1.0, 0.0)
      );
    }
    case 4u: { // √X
      return array<vec2<f32>, 4>(
        vec2<f32>(0.5, 0.5),
        vec2<f32>(0.5, -0.5),
        vec2<f32>(0.5, -0.5),
        vec2<f32>(0.5, 0.5)
      );
    }
    case 5u: { // S
      return array<vec2<f32>, 4>(
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 1.0)
      );
    }
    case 6u: { // S†
      return array<vec2<f32>, 4>(
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, -1.0)
      );
    }
    case 7u: { // T
      return array<vec2<f32>, 4>(
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(invSqrt2, invSqrt2)
      );
    }
    case 8u: { // T†
      return array<vec2<f32>, 4>(
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(invSqrt2, -invSqrt2)
      );
    }
    default: {
      return array<vec2<f32>, 4>(
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0)
      );
    }
  }
}

@compute @workgroup_size(1)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x > 0u) {
    return;
  }

  let a00 = inputState[0];
  let a01 = inputState[1];
  let a10 = inputState[2];
  let a11 = inputState[3];
  let m = gate_matrix(params.gateType);
  let m00 = m[0];
  let m01 = m[1];
  let m10 = m[2];
  let m11 = m[3];

  if (params.targetIndex == 0u) {
    let out00 = cadd(cmul(m00, a00), cmul(m01, a10));
    let out10 = cadd(cmul(m10, a00), cmul(m11, a10));
    let out01 = cadd(cmul(m00, a01), cmul(m01, a11));
    let out11 = cadd(cmul(m10, a01), cmul(m11, a11));
    outputState[0] = out00;
    outputState[1] = out01;
    outputState[2] = out10;
    outputState[3] = out11;
  } else {
    let out00 = cadd(cmul(m00, a00), cmul(m01, a01));
    let out01 = cadd(cmul(m10, a00), cmul(m11, a01));
    let out10 = cadd(cmul(m00, a10), cmul(m01, a11));
    let out11 = cadd(cmul(m10, a10), cmul(m11, a11));
    outputState[0] = out00;
    outputState[1] = out01;
    outputState[2] = out10;
    outputState[3] = out11;
  }
}
`

export const stateTextComputeCode = `
const STATE_TEXT_LEN: u32 = ${STATE_TEXT_MAX_LEN}u;
const EPSILON: f32 = 0.0000005;
const PI: f32 = 3.14159265;
const RAD_TO_DEG: f32 = 180.0 / PI;
const LINE1_START: u32 = 0u;
const LINE2_START: u32 = 14u;
const LINE3_START: u32 = 42u;
const LINE4_START: u32 = 65u;

@group(0) @binding(0) var<storage, read> stateVector: array<vec2<f32>, 4>;
@group(0) @binding(1) var<storage, read_write> glyphs: array<u32>;

fn write_char(idx: ptr<function, u32>, code: u32) {
  if (*idx < STATE_TEXT_LEN) {
    glyphs[*idx] = code;
  }
  *idx = *idx + 1u;
}

fn write_digit(idx: ptr<function, u32>, digit: u32) {
  let d = min(digit, 9u);
  write_char(idx, 48u + d);
}

fn write_uint_padded(idx: ptr<function, u32>, value: u32, width: u32) {
  var div: u32 = 1u;
  for (var i: u32 = 1u; i < width; i = i + 1u) {
    div = div * 10u;
  }
  var v = value;
  for (var i: u32 = 0u; i < width; i = i + 1u) {
    let digit = v / div;
    write_digit(idx, digit);
    v = v - digit * div;
    if (div > 1u) {
      div = div / 10u;
    }
  }
}

fn write_fixed_unsigned(idx: ptr<function, u32>, value: f32, int_width: u32, decimals: u32) {
  let int_part = u32(value);
  write_uint_padded(idx, int_part, int_width);
  write_char(idx, 46u);

  var frac = value - f32(int_part);
  for (var i: u32 = 0u; i < decimals; i = i + 1u) {
    frac = frac * 10.0;
    let digit = u32(frac);
    write_digit(idx, digit);
    frac = frac - f32(digit);
  }
}

fn write_fixed_signed(idx: ptr<function, u32>, value: f32, int_width: u32, decimals: u32) {
  let sign = select(43u, 45u, value < 0.0);
  write_char(idx, sign);
  write_fixed_unsigned(idx, abs(value), int_width, decimals);
}

fn sanitize(value: f32) -> f32 {
  return select(value, 0.0, abs(value) < EPSILON);
}

@compute @workgroup_size(1)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x > 0u) {
    return;
  }

  for (var i: u32 = 0u; i < STATE_TEXT_LEN; i = i + 1u) {
    glyphs[i] = 0u;
  }

  let a0 = stateVector[0];
  let re = sanitize(a0.x);
  let im = sanitize(a0.y);
  let prob = (re * re + im * im) * 100.0;
  let phase = sanitize(atan2(im, re) * RAD_TO_DEG);

  var cursor: u32 = LINE1_START;
  write_char(&cursor, 124u);
  write_char(&cursor, 48u);
  write_char(&cursor, 48u);
  write_char(&cursor, 62u);
  write_char(&cursor, 32u);
  write_char(&cursor, 68u);
  write_char(&cursor, 69u);
  write_char(&cursor, 67u);
  write_char(&cursor, 73u);
  write_char(&cursor, 77u);
  write_char(&cursor, 65u);
  write_char(&cursor, 76u);
  write_char(&cursor, 32u);
  write_char(&cursor, 48u);

  cursor = LINE2_START;
  write_char(&cursor, 65u);
  write_char(&cursor, 77u);
  write_char(&cursor, 80u);
  write_char(&cursor, 76u);
  write_char(&cursor, 73u);
  write_char(&cursor, 84u);
  write_char(&cursor, 85u);
  write_char(&cursor, 68u);
  write_char(&cursor, 69u);
  write_char(&cursor, 58u);
  write_char(&cursor, 32u);
  write_fixed_signed(&cursor, re, 1u, 5u);
  let im_sign = select(43u, 45u, im < 0.0);
  write_char(&cursor, im_sign);
  write_fixed_unsigned(&cursor, abs(im), 1u, 5u);
  write_char(&cursor, 105u);

  cursor = LINE3_START;
  write_char(&cursor, 80u);
  write_char(&cursor, 82u);
  write_char(&cursor, 79u);
  write_char(&cursor, 66u);
  write_char(&cursor, 65u);
  write_char(&cursor, 66u);
  write_char(&cursor, 73u);
  write_char(&cursor, 76u);
  write_char(&cursor, 73u);
  write_char(&cursor, 84u);
  write_char(&cursor, 89u);
  write_char(&cursor, 58u);
  write_char(&cursor, 32u);
  write_fixed_signed(&cursor, prob, 3u, 4u);
  write_char(&cursor, 37u);

  cursor = LINE4_START;
  write_char(&cursor, 80u);
  write_char(&cursor, 72u);
  write_char(&cursor, 65u);
  write_char(&cursor, 83u);
  write_char(&cursor, 69u);
  write_char(&cursor, 58u);
  write_char(&cursor, 32u);
  write_fixed_signed(&cursor, phase, 3u, 2u);
  write_char(&cursor, 68u);
  write_char(&cursor, 69u);
  write_char(&cursor, 71u);
}
`

export const shapeShaderCode = `
struct Uniforms {
  resolution: vec2<f32>,
  _pad: vec2<f32>,
}

struct Instance {
  kind: u32,
  _pad0: u32,
  _pad1: u32,
  _pad2: u32,
  thickness: f32,
  _pad3: vec3<f32>,
  p0: vec2<f32>,
  p1: vec2<f32>,
  color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexOut {
  @builtin(position) position: vec4<f32>,
  @location(0) color: vec4<f32>,
  @location(1) localPos: vec2<f32>,
  @location(2) size: vec2<f32>,
  @location(3) kind: f32,
  @location(4) radius: f32,
}

fn to_clip(pos: vec2<f32>) -> vec4<f32> {
  let x = (pos.x / uniforms.resolution.x) * 2.0 - 1.0;
  let y = 1.0 - (pos.y / uniforms.resolution.y) * 2.0;
  return vec4<f32>(x, y, 0.0, 1.0);
}

@vertex
fn vs_main(@builtin(vertex_index) vertexIndex: u32, @builtin(instance_index) instanceIndex: u32,
           @location(0) kind: f32,
           @location(1) thickness: f32,
           @location(2) p0: vec2<f32>,
           @location(3) p1: vec2<f32>,
           @location(4) color: vec4<f32>) -> VertexOut {
  var out: VertexOut;
  let quad = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 0.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(0.0, 0.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(0.0, 1.0)
  );
  let local = quad[vertexIndex];
  var pos = vec2<f32>(0.0, 0.0);

  let kindValue = u32(round(kind));
  if (kindValue == 0u) {
    pos = p0 + local * p1;
  } else {
    if (kindValue == 1u) {
      let dir = normalize(p1 - p0);
      let normal = vec2<f32>(-dir.y, dir.x) * (thickness * 0.5);
      let a = p0 + normal;
      let b = p0 - normal;
      let c = p1 - normal;
      let d = p1 + normal;
      let quadPts = array<vec2<f32>, 6>(a, b, c, a, c, d);
      pos = quadPts[vertexIndex];
    } else {
      pos = p0 + local * p1;
    }
  }

  out.position = to_clip(pos);
  out.color = color;
  out.localPos = local * p1;
  out.size = p1;
  out.kind = kind;
  out.radius = thickness;
  return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
  let kindValue = u32(round(input.kind));
  if (kindValue == 2u) {
    let half = input.size * 0.5;
    let p = input.localPos - half;
    let radius = min(input.radius, min(half.x, half.y));
    let q = abs(p) - (half - vec2<f32>(radius, radius));
    let dist = length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - radius;
    let aa = 1.0;
    let alpha = smoothstep(aa, 0.0, dist);
    return vec4<f32>(input.color.rgb, input.color.a * alpha);
  }
  return input.color;
}
`

export const textShaderCode = `
struct TextUniforms {
  resolution: vec2<f32>,
  basePos: vec2<f32>,
  glyphSize: vec2<f32>,
  atlasSize: vec2<f32>,
  color: vec4<f32>,
  glyphOffset: vec2<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: TextUniforms;
@group(0) @binding(1) var<storage, read> glyphs: array<u32>;
@group(0) @binding(2) var fontSampler: sampler;
@group(0) @binding(3) var fontTexture: texture_2d<f32>;

struct VertexOut {
  @builtin(position) position: vec4<f32>,
  @location(0) uv: vec2<f32>,
  @location(1) color: vec4<f32>,
  @location(2) valid: f32,
}

@vertex
fn vs_main(@builtin(vertex_index) vertexIndex: u32, @builtin(instance_index) instanceIndex: u32) -> VertexOut {
  var out: VertexOut;
  let quad = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 0.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(0.0, 0.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(0.0, 1.0)
  );
  let local = quad[vertexIndex];

  let bufferIndex = instanceIndex + u32(round(uniforms.glyphOffset.x));
  let code = glyphs[bufferIndex];
  var glyphIndex = i32(code) - 32;
  let maxGlyphs = 16 * 6;
  var valid = 1.0;
  if (glyphIndex < 0 || glyphIndex >= maxGlyphs) {
    glyphIndex = 0;
    valid = 0.0;
  }
  let col = glyphIndex % 16;
  let row = glyphIndex / 16;

  let pos = vec2<f32>(
    uniforms.basePos.x + f32(instanceIndex) * uniforms.glyphSize.x + local.x * uniforms.glyphSize.x,
    uniforms.basePos.y + local.y * uniforms.glyphSize.y
  );
  let clipX = (pos.x / uniforms.resolution.x) * 2.0 - 1.0;
  let clipY = 1.0 - (pos.y / uniforms.resolution.y) * 2.0;
  out.position = vec4<f32>(clipX, clipY, 0.0, 1.0);

  let uv = (vec2<f32>(f32(col), f32(row)) + local) * (uniforms.glyphSize / uniforms.atlasSize);
  out.uv = uv;
  out.color = uniforms.color;
  out.valid = valid;
  return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
  let sampled = textureSample(fontTexture, fontSampler, input.uv);
  let alpha = sampled.a * input.color.a * input.valid;
  return vec4<f32>(input.color.rgb, alpha);
}
`
