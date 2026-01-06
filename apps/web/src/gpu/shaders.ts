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

export const stateCircleComputeCode = `
struct CircleLayout {
  baseSizeGap: vec4<f32>,
  strokePad: vec4<f32>,
  fillColor: vec4<f32>,
  outlineColor: vec4<f32>,
  outlineZeroColor: vec4<f32>,
  innerColor: vec4<f32>,
}

struct Instance {
  data: array<f32, 11>,
}

@group(0) @binding(0) var<storage, read> stateVector: array<vec2<f32>, 4>;
@group(0) @binding(1) var<storage, read_write> instances: array<Instance>;
@group(0) @binding(2) var<uniform> circleLayout: CircleLayout;

fn write_instance(index: u32, kind: f32, thickness: f32, p0: vec2<f32>, p1: vec2<f32>, color: vec4<f32>) {
  instances[index].data[0] = kind;
  instances[index].data[1] = thickness;
  instances[index].data[2] = p0.x;
  instances[index].data[3] = p0.y;
  instances[index].data[4] = p1.x;
  instances[index].data[5] = p1.y;
  instances[index].data[6] = color.x;
  instances[index].data[7] = color.y;
  instances[index].data[8] = color.z;
  instances[index].data[9] = color.w;
  instances[index].data[10] = 0.0;
}

@compute @workgroup_size(1)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x > 0u) {
    return;
  }

  let base = circleLayout.baseSizeGap.xy;
  let size = circleLayout.baseSizeGap.z;
  let gap = circleLayout.baseSizeGap.w;
  let stroke = circleLayout.strokePad.x;
  let innerSize = max(size - stroke * 2.0, 0.0);
  let radius = size * 0.5;
  let innerRadius = innerSize * 0.5;

  for (var i: u32 = 0u; i < 4u; i = i + 1u) {
    let amplitude = stateVector[i];
    let probability = clamp(dot(amplitude, amplitude), 0.0, 1.0);
    let fillSize = innerSize * sqrt(probability);
    let fillRadius = fillSize * 0.5;
    let x = base.x + f32(i) * (size + gap);
    let y = base.y;
    let outlineColor = select(circleLayout.outlineZeroColor, circleLayout.outlineColor, probability > 0.0);
    let instanceBase = i * 3u;

    write_instance(
      instanceBase,
      2.0,
      radius,
      vec2<f32>(x, y),
      vec2<f32>(size, size),
      outlineColor
    );

    write_instance(
      instanceBase + 1u,
      2.0,
      innerRadius,
      vec2<f32>(x + stroke, y + stroke),
      vec2<f32>(innerSize, innerSize),
      circleLayout.innerColor
    );

    write_instance(
      instanceBase + 2u,
      2.0,
      fillRadius,
      vec2<f32>(x + (size - fillSize) * 0.5, y + (size - fillSize) * 0.5),
      vec2<f32>(fillSize, fillSize),
      circleLayout.fillColor
    );
  }
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
