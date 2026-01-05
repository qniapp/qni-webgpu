import './style.css'
import { gateToIndex, getGateFromQuery, type Gate } from './domain/gate'

declare global {
  interface Window {
    __renderDone?: boolean
    __vertexCount?: number
    __debugPixel?: number[]
    __captureFrame?: boolean
    __captureStateVector?: boolean
    __frameDataUrl?: string
    __stateVector?: number[]
  }
}

const CANVAS_WIDTH = 800
const CANVAS_HEIGHT = 600
const STATE_TEXT_MAX_LEN = 50

const app = document.querySelector<HTMLDivElement>('#app')
if (!app) {
  throw new Error('#app not found')
}

app.innerHTML = `
  <canvas id="gfx" width="${CANVAS_WIDTH}" height="${CANVAS_HEIGHT}"></canvas>
  <div id="status" aria-live="polite"></div>
`

const statusEl = document.querySelector<HTMLDivElement>('#status')
const canvas = document.querySelector<HTMLCanvasElement>('#gfx')
if (!canvas) {
  throw new Error('#gfx not found')
}

function setStatus(message: string) {
  if (statusEl) {
    statusEl.textContent = message
  }
}

if (!navigator.gpu) {
  setStatus('WebGPU is not supported in this browser.')
  throw new Error('WebGPU not supported')
}

type Color = [number, number, number, number]

const COLORS = {
  background: [0.06, 0.06, 0.07, 1.0] as Color,
  line: [0.9, 0.9, 0.92, 1.0] as Color,
  box: [0.18, 0.18, 0.2, 1.0] as Color,
  boxBorder: [0.92, 0.92, 0.95, 1.0] as Color,
  text: [0.92, 0.92, 0.95, 1.0] as Color,
}

const computeShaderCode = `
struct GateParams {
  gateType: u32,
  _pad0: u32,
  _pad1: u32,
  _pad2: u32,
}

@group(0) @binding(0) var<uniform> params: GateParams;
@group(0) @binding(1) var<storage, read> inputState: array<vec2<f32>, 2>;
@group(0) @binding(2) var<storage, read_write> outputState: array<vec2<f32>, 2>;

fn cadd(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
  return a + b;
}

fn csub(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
  return a - b;
}

fn cmul(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
  return vec2<f32>(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}

@compute @workgroup_size(1)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x > 0u) {
    return;
  }

  let a0 = inputState[0];
  let a1 = inputState[1];
  var out0 = a0;
  var out1 = a1;
  let invSqrt2 = 0.7071067811865476;

  switch params.gateType {
    case 0u: { // X
      out0 = a1;
      out1 = a0;
    }
    case 1u: { // H
      out0 = cadd(a0, a1) * invSqrt2;
      out1 = csub(a0, a1) * invSqrt2;
    }
    case 2u: { // Y
      out0 = cmul(vec2<f32>(0.0, -1.0), a1);
      out1 = cmul(vec2<f32>(0.0, 1.0), a0);
    }
    case 3u: { // Z
      out0 = a0;
      out1 = -a1;
    }
    case 4u: { // S
      out0 = a0;
      out1 = cmul(vec2<f32>(0.0, 1.0), a1);
    }
    case 5u: { // T
      out0 = a0;
      out1 = cmul(vec2<f32>(invSqrt2, invSqrt2), a1);
    }
    default: {
      out0 = a0;
      out1 = a1;
    }
  }

  outputState[0] = out0;
  outputState[1] = out1;
}
`

const stateTextComputeCode = `
const STATE_TEXT_LEN: u32 = ${STATE_TEXT_MAX_LEN}u;
const EPSILON: f32 = 0.000001;
const INV_SQRT2: f32 = 0.7071067811865476;

const TEXT_X: array<u32, ${STATE_TEXT_MAX_LEN}> = array<u32, ${STATE_TEXT_MAX_LEN}>(
  91u, 40u, 48u, 43u, 48u, 105u, 41u, 44u, 32u, 40u, 49u, 43u, 48u, 105u, 41u, 93u,
  0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u,
  0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u
);

const TEXT_H: array<u32, ${STATE_TEXT_MAX_LEN}> = array<u32, ${STATE_TEXT_MAX_LEN}>(
  91u, 40u, 48u, 46u, 55u, 48u, 55u, 49u, 48u, 54u, 55u, 56u, 49u, 49u, 56u, 54u,
  53u, 52u, 55u, 53u, 43u, 48u, 105u, 41u, 44u, 32u, 40u, 48u, 46u, 55u, 48u, 55u,
  49u, 48u, 54u, 55u, 56u, 49u, 49u, 56u, 54u, 53u, 52u, 55u, 53u, 43u, 48u, 105u,
  41u, 93u
);

const TEXT_Y: array<u32, ${STATE_TEXT_MAX_LEN}> = array<u32, ${STATE_TEXT_MAX_LEN}>(
  91u, 40u, 48u, 43u, 48u, 105u, 41u, 44u, 32u, 40u, 48u, 43u, 49u, 105u, 41u, 93u,
  0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u,
  0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u
);

const TEXT_Z: array<u32, ${STATE_TEXT_MAX_LEN}> = array<u32, ${STATE_TEXT_MAX_LEN}>(
  91u, 40u, 49u, 43u, 48u, 105u, 41u, 44u, 32u, 40u, 48u, 43u, 48u, 105u, 41u, 93u,
  0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u,
  0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u
);

@group(0) @binding(0) var<storage, read> stateVector: array<vec2<f32>, 2>;
@group(0) @binding(1) var<storage, read_write> glyphs: array<u32>;

fn close(a: f32, b: f32) -> bool {
  return abs(a - b) < EPSILON;
}

@compute @workgroup_size(64)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let idx = gid.x;
  if (idx >= STATE_TEXT_LEN) {
    return;
  }

  let a0 = stateVector[0];
  let a1 = stateVector[1];
  let isX = close(a0.x, 0.0) && close(a0.y, 0.0) && close(a1.x, 1.0) && close(a1.y, 0.0);
  let isH = close(a0.x, INV_SQRT2) && close(a0.y, 0.0) && close(a1.x, INV_SQRT2) && close(a1.y, 0.0);
  let isY = close(a0.x, 0.0) && close(a0.y, 0.0) && close(a1.x, 0.0) && close(a1.y, 1.0);
  let isZ = close(a0.x, 1.0) && close(a0.y, 0.0) && close(a1.x, 0.0) && close(a1.y, 0.0);

  var code = 0u;
  if (isH) {
    code = TEXT_H[idx];
  } else if (isX) {
    code = TEXT_X[idx];
  } else if (isY) {
    code = TEXT_Y[idx];
  } else if (isZ) {
    code = TEXT_Z[idx];
  }

  glyphs[idx] = code;
}
`

const shapeShaderCode = `
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
    let dir = normalize(p1 - p0);
    let normal = vec2<f32>(-dir.y, dir.x) * (thickness * 0.5);
    let a = p0 + normal;
    let b = p0 - normal;
    let c = p1 - normal;
    let d = p1 + normal;
    let quadPts = array<vec2<f32>, 6>(a, b, c, a, c, d);
    pos = quadPts[vertexIndex];
  }

  out.position = to_clip(pos);
  out.color = color;
  return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
  return input.color;
}
`

const textShaderCode = `
struct TextUniforms {
  resolution: vec2<f32>,
  basePos: vec2<f32>,
  glyphSize: vec2<f32>,
  atlasSize: vec2<f32>,
  color: vec4<f32>,
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

  let code = glyphs[instanceIndex];
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

type ShapeInstance = {
  kind: 0 | 1
  thickness: number
  p0x: number
  p0y: number
  p1x: number
  p1y: number
  color: Color
}

const instances: ShapeInstance[] = []

function addRect(x: number, y: number, w: number, h: number, color: Color) {
  instances.push({
    kind: 0,
    thickness: 0,
    p0x: x,
    p0y: y,
    p1x: w,
    p1y: h,
    color,
  })
}

function addLine(x1: number, y1: number, x2: number, y2: number, thickness: number, color: Color) {
  instances.push({
    kind: 1,
    thickness,
    p0x: x1,
    p0y: y1,
    p1x: x2,
    p1y: y2,
    color,
  })
}

const FONT_GLYPH_SIZE = 8
const FONT_COLS = 16
const FONT_ROWS = 6
const FONT_ATLAS_WIDTH = FONT_COLS * FONT_GLYPH_SIZE
const FONT_ATLAS_HEIGHT = FONT_ROWS * FONT_GLYPH_SIZE

function createFontAtlas() {
  const data = new Uint8Array(FONT_ATLAS_WIDTH * FONT_ATLAS_HEIGHT * 4)

  const glyphs: Record<string, string[]> = {
    'H': [
      '11000011',
      '11000011',
      '11000011',
      '11111111',
      '11000011',
      '11000011',
      '11000011',
      '00000000',
    ],
    'X': [
      '11000011',
      '01100110',
      '00111100',
      '00011000',
      '00111100',
      '01100110',
      '11000011',
      '00000000',
    ],
    'Y': [
      '11000011',
      '01100110',
      '00111100',
      '00011000',
      '00011000',
      '00011000',
      '00011000',
      '00000000',
    ],
    'Z': [
      '11111111',
      '00000110',
      '00001100',
      '00011000',
      '00110000',
      '01100000',
      '11111111',
      '00000000',
    ],
    'S': [
      '01111110',
      '11000000',
      '11000000',
      '01111110',
      '00000011',
      '00000011',
      '11111110',
      '00000000',
    ],
    'T': [
      '11111111',
      '00110000',
      '00110000',
      '00110000',
      '00110000',
      '00110000',
      '00110000',
      '00000000',
    ],
    '0': [
      '01111110',
      '11000011',
      '11000111',
      '11011011',
      '11110011',
      '11000011',
      '01111110',
      '00000000',
    ],
    '1': [
      '00110000',
      '01110000',
      '00110000',
      '00110000',
      '00110000',
      '00110000',
      '01111000',
      '00000000',
    ],
    '2': [
      '01111110',
      '11000011',
      '00000011',
      '00001110',
      '00111000',
      '11100000',
      '11111111',
      '00000000',
    ],
    '3': [
      '01111110',
      '11000011',
      '00000011',
      '00111110',
      '00000011',
      '11000011',
      '01111110',
      '00000000',
    ],
    '4': [
      '00011100',
      '00111100',
      '01101100',
      '11001100',
      '11111111',
      '00001100',
      '00001100',
      '00000000',
    ],
    '5': [
      '11111111',
      '11000000',
      '11111110',
      '00000011',
      '00000011',
      '11000011',
      '01111110',
      '00000000',
    ],
    '6': [
      '00111110',
      '01100000',
      '11000000',
      '11111110',
      '11000011',
      '11000011',
      '01111110',
      '00000000',
    ],
    '7': [
      '11111111',
      '00000011',
      '00000110',
      '00001100',
      '00011000',
      '00110000',
      '00110000',
      '00000000',
    ],
    '8': [
      '01111110',
      '11000011',
      '11000011',
      '01111110',
      '11000011',
      '11000011',
      '01111110',
      '00000000',
    ],
    '9': [
      '01111110',
      '11000011',
      '11000011',
      '01111111',
      '00000011',
      '00000110',
      '01111100',
      '00000000',
    ],
    '+': [
      '00000000',
      '00011000',
      '00011000',
      '01111110',
      '00011000',
      '00011000',
      '00000000',
      '00000000',
    ],
    '-': [
      '00000000',
      '00000000',
      '00000000',
      '01111110',
      '00000000',
      '00000000',
      '00000000',
      '00000000',
    ],
    '.': [
      '00000000',
      '00000000',
      '00000000',
      '00000000',
      '00000000',
      '00011000',
      '00011000',
      '00000000',
    ],
    'i': [
      '00011000',
      '00000000',
      '00011000',
      '00011000',
      '00011000',
      '00011000',
      '00011100',
      '00000000',
    ],
    '(': [
      '00001100',
      '00011000',
      '00110000',
      '00110000',
      '00110000',
      '00011000',
      '00001100',
      '00000000',
    ],
    ')': [
      '00110000',
      '00011000',
      '00001100',
      '00001100',
      '00001100',
      '00011000',
      '00110000',
      '00000000',
    ],
    '[': [
      '00111100',
      '00110000',
      '00110000',
      '00110000',
      '00110000',
      '00110000',
      '00111100',
      '00000000',
    ],
    ']': [
      '00111100',
      '00001100',
      '00001100',
      '00001100',
      '00001100',
      '00001100',
      '00111100',
      '00000000',
    ],
    ',': [
      '00000000',
      '00000000',
      '00000000',
      '00000000',
      '00011000',
      '00011000',
      '00110000',
      '00000000',
    ],
    ' ': [
      '00000000',
      '00000000',
      '00000000',
      '00000000',
      '00000000',
      '00000000',
      '00000000',
      '00000000',
    ],
  }

  const setGlyph = (char: string, rows: string[]) => {
    const code = char.charCodeAt(0)
    const index = code - 32
    if (index < 0 || index >= FONT_COLS * FONT_ROWS) {
      return
    }
    const col = index % FONT_COLS
    const row = Math.floor(index / FONT_COLS)
    const baseX = col * FONT_GLYPH_SIZE
    const baseY = row * FONT_GLYPH_SIZE
    rows.forEach((rowBits, y) => {
      for (let x = 0; x < FONT_GLYPH_SIZE; x += 1) {
        const value = rowBits[x] === '1' ? 255 : 0
        const px = baseX + x
        const py = baseY + y
        const offset = (py * FONT_ATLAS_WIDTH + px) * 4
        data[offset] = 255
        data[offset + 1] = 255
        data[offset + 2] = 255
        data[offset + 3] = value
      }
    })
  }

  Object.entries(glyphs).forEach(([char, rows]) => setGlyph(char, rows))

  return data
}

type TextLayout = {
  text: string
  x: number
  y: number
  color: Color
  glyphCount?: number
}

function buildScene(
  gateLabel: Gate,
  stateVectorGlyphCount: number
): { gateLabel: TextLayout; stateVector: TextLayout } {
  instances.length = 0
  const lineY = 160
  const lineLeft = 80
  const lineRight = CANVAS_WIDTH - 80
  addRect(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT, COLORS.background)
  addRect(lineLeft, lineY - 2, lineRight - lineLeft, 4, COLORS.line)

  const gateSize = 60
  const gateX = (lineLeft + lineRight) / 2 - gateSize / 2
  const gateY = lineY - gateSize / 2
  addRect(gateX, gateY, gateSize, gateSize, COLORS.box)
  const border = 3
  addRect(gateX, gateY, gateSize, border, COLORS.boxBorder)
  addRect(gateX, gateY + gateSize - border, gateSize, border, COLORS.boxBorder)
  addRect(gateX, gateY, border, gateSize, COLORS.boxBorder)
  addRect(gateX + gateSize - border, gateY, border, gateSize, COLORS.boxBorder)

  const gateLabelX = gateX + gateSize / 2 - FONT_GLYPH_SIZE / 2
  const gateLabelY = gateY + gateSize / 2 - FONT_GLYPH_SIZE / 2

  window.__vertexCount = instances.length

  return {
    gateLabel: {
      text: gateLabel,
      x: gateLabelX,
      y: gateLabelY,
      color: COLORS.boxBorder,
    },
    stateVector: {
      text: '',
      x: 120,
      y: 320,
      color: COLORS.text,
      glyphCount: stateVectorGlyphCount,
    },
  }
}

async function computeStateVector(
  device: GPUDevice,
  gate: Gate,
  readback: boolean
): Promise<{ outputBuffer: GPUBuffer; readback: Float32Array | null }> {
  const inputState = new Float32Array([1, 0, 0, 0])
  const inputBuffer = device.createBuffer({
    size: inputState.byteLength,
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
  })
  device.queue.writeBuffer(inputBuffer, 0, inputState)

  const outputBuffer = device.createBuffer({
    size: inputState.byteLength,
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
  })

  const paramsBuffer = device.createBuffer({
    size: 16,
    usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
  })
  device.queue.writeBuffer(paramsBuffer, 0, new Uint32Array([gateToIndex(gate), 0, 0, 0]))

  const shaderModule = device.createShaderModule({ code: computeShaderCode })
  const compilationInfo = await shaderModule.getCompilationInfo()
  const compilationErrors = compilationInfo.messages.filter((message) => message.type === 'error')
  if (compilationErrors.length > 0) {
    const message = compilationErrors.map((error) => error.message).join(' | ')
    throw new Error(message)
  }

  const pipeline = device.createComputePipeline({
    layout: 'auto',
    compute: {
      module: shaderModule,
      entryPoint: 'cs_main',
    },
  })

  const bindGroup = device.createBindGroup({
    layout: pipeline.getBindGroupLayout(0),
    entries: [
      { binding: 0, resource: { buffer: paramsBuffer } },
      { binding: 1, resource: { buffer: inputBuffer } },
      { binding: 2, resource: { buffer: outputBuffer } },
    ],
  })

  const commandEncoder = device.createCommandEncoder()
  const pass = commandEncoder.beginComputePass()
  pass.setPipeline(pipeline)
  pass.setBindGroup(0, bindGroup)
  pass.dispatchWorkgroups(1)
  pass.end()
  if (readback) {
    const readbackBuffer = device.createBuffer({
      size: inputState.byteLength,
      usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
    })
    commandEncoder.copyBufferToBuffer(outputBuffer, 0, readbackBuffer, 0, inputState.byteLength)
    device.queue.submit([commandEncoder.finish()])
    await readbackBuffer.mapAsync(GPUMapMode.READ)
    const mapped = new Float32Array(readbackBuffer.getMappedRange())
    const result = new Float32Array(mapped)
    readbackBuffer.unmap()
    return { outputBuffer, readback: result }
  }
  device.queue.submit([commandEncoder.finish()])
  return { outputBuffer, readback: null }
}

async function populateStateTextBuffer(
  device: GPUDevice,
  stateVectorBuffer: GPUBuffer,
  glyphBuffer: GPUBuffer
): Promise<void> {
  const shaderModule = device.createShaderModule({ code: stateTextComputeCode })
  const compilationInfo = await shaderModule.getCompilationInfo()
  const compilationErrors = compilationInfo.messages.filter((message) => message.type === 'error')
  if (compilationErrors.length > 0) {
    const message = compilationErrors.map((error) => error.message).join(' | ')
    throw new Error(message)
  }

  const pipeline = device.createComputePipeline({
    layout: 'auto',
    compute: {
      module: shaderModule,
      entryPoint: 'cs_main',
    },
  })

  const bindGroup = device.createBindGroup({
    layout: pipeline.getBindGroupLayout(0),
    entries: [
      { binding: 0, resource: { buffer: stateVectorBuffer } },
      { binding: 1, resource: { buffer: glyphBuffer } },
    ],
  })

  const commandEncoder = device.createCommandEncoder()
  const pass = commandEncoder.beginComputePass()
  pass.setPipeline(pipeline)
  pass.setBindGroup(0, bindGroup)
  pass.dispatchWorkgroups(Math.ceil(STATE_TEXT_MAX_LEN / 64))
  pass.end()
  device.queue.submit([commandEncoder.finish()])
}

async function init() {
  try {
    const adapter = await navigator.gpu.requestAdapter()
    if (!adapter) {
      setStatus('WebGPU adapter not found.')
      throw new Error('No WebGPU adapter')
    }
    const device = await adapter.requestDevice()
    device.onuncapturederror = (event) => {
      setStatus(event.error.message)
    }
    const context = canvas.getContext('webgpu')
    if (!context) {
      setStatus('WebGPU context not available.')
      throw new Error('WebGPU context unavailable')
    }
    const presentationFormat = navigator.gpu.getPreferredCanvasFormat()
    context.configure({
      device,
      format: presentationFormat,
      alphaMode: 'opaque',
      usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
    })

    const gateLabel = getGateFromQuery()
    const shouldReadback = window.__captureStateVector === true
    const { outputBuffer: stateVectorBuffer, readback: stateVectorReadback } = await computeStateVector(
      device,
      gateLabel,
      shouldReadback
    )
    if (stateVectorReadback) {
      window.__stateVector = Array.from(stateVectorReadback)
    }

    const stateTextGlyphBuffer = device.createBuffer({
      size: STATE_TEXT_MAX_LEN * 4,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
    })
    device.queue.writeBuffer(stateTextGlyphBuffer, 0, new Uint32Array(STATE_TEXT_MAX_LEN))
    await populateStateTextBuffer(device, stateVectorBuffer, stateTextGlyphBuffer)

    const fontData = createFontAtlas()
    const bytesPerPixel = 4
    const unpaddedBytesPerRow = FONT_ATLAS_WIDTH * bytesPerPixel
    const paddedBytesPerRow = Math.ceil(unpaddedBytesPerRow / 256) * 256
    const paddedFontData = new Uint8Array(paddedBytesPerRow * FONT_ATLAS_HEIGHT)
    for (let row = 0; row < FONT_ATLAS_HEIGHT; row += 1) {
      const srcOffset = row * unpaddedBytesPerRow
      const dstOffset = row * paddedBytesPerRow
      paddedFontData.set(fontData.subarray(srcOffset, srcOffset + unpaddedBytesPerRow), dstOffset)
    }
    const fontTexture = device.createTexture({
      size: [FONT_ATLAS_WIDTH, FONT_ATLAS_HEIGHT, 1],
      format: 'rgba8unorm',
      usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
    })
    device.queue.writeTexture(
      { texture: fontTexture },
      paddedFontData,
      { bytesPerRow: paddedBytesPerRow, rowsPerImage: FONT_ATLAS_HEIGHT },
      { width: FONT_ATLAS_WIDTH, height: FONT_ATLAS_HEIGHT }
    )
    const fontSampler = device.createSampler({
      magFilter: 'nearest',
      minFilter: 'nearest',
    })

    const textLayout = buildScene(gateLabel, STATE_TEXT_MAX_LEN)

    const instanceStride = 11
    const instanceData = new Float32Array(instances.length * instanceStride)
    instances.forEach((instance, index) => {
      const offset = index * instanceStride
      instanceData[offset] = instance.kind
      instanceData[offset + 1] = instance.thickness
      instanceData[offset + 2] = instance.p0x
      instanceData[offset + 3] = instance.p0y
      instanceData[offset + 4] = instance.p1x
      instanceData[offset + 5] = instance.p1y
      instanceData[offset + 6] = instance.color[0]
      instanceData[offset + 7] = instance.color[1]
      instanceData[offset + 8] = instance.color[2]
      instanceData[offset + 9] = instance.color[3]
      instanceData[offset + 10] = 0
    })
    const instanceBuffer = device.createBuffer({
      size: instanceData.byteLength,
      usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
    })
    device.queue.writeBuffer(instanceBuffer, 0, instanceData.buffer)

    const uniformBuffer = device.createBuffer({
      size: 16,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    })
    device.queue.writeBuffer(uniformBuffer, 0, new Float32Array([CANVAS_WIDTH, CANVAS_HEIGHT, 0, 0]))

    const shaderModule = device.createShaderModule({ code: shapeShaderCode })
    const compilationInfo = await shaderModule.getCompilationInfo()
    const compilationErrors = compilationInfo.messages.filter((message) => message.type === 'error')
    if (compilationErrors.length > 0) {
      const message = compilationErrors.map((error) => error.message).join(' | ')
      setStatus(message)
      throw new Error(message)
    }
    device.pushErrorScope('validation')
    const pipeline = device.createRenderPipeline({
      layout: 'auto',
      vertex: {
        module: shaderModule,
        entryPoint: 'vs_main',
        buffers: [
          {
            arrayStride: instanceStride * 4,
            stepMode: 'instance',
            attributes: [
              { shaderLocation: 0, offset: 0, format: 'float32' },
              { shaderLocation: 1, offset: 1 * 4, format: 'float32' },
              { shaderLocation: 2, offset: 2 * 4, format: 'float32x2' },
              { shaderLocation: 3, offset: 4 * 4, format: 'float32x2' },
              { shaderLocation: 4, offset: 6 * 4, format: 'float32x4' },
            ],
          },
        ],
      },
      fragment: {
        module: shaderModule,
        entryPoint: 'fs_main',
        targets: [
          {
            format: presentationFormat,
            blend: {
              color: {
                srcFactor: 'src-alpha',
                dstFactor: 'one-minus-src-alpha',
                operation: 'add',
              },
              alpha: {
                srcFactor: 'one',
                dstFactor: 'one-minus-src-alpha',
                operation: 'add',
              },
            },
          },
        ],
      },
      primitive: {
        topology: 'triangle-list',
      },
    })
    const pipelineError = await device.popErrorScope()
    if (pipelineError) {
      setStatus(pipelineError.message)
      throw pipelineError
    }

    const bindGroup = device.createBindGroup({
      layout: pipeline.getBindGroupLayout(0),
      entries: [{ binding: 0, resource: { buffer: uniformBuffer } }],
    })

    const textShaderModule = device.createShaderModule({ code: textShaderCode })
    const textCompilationInfo = await textShaderModule.getCompilationInfo()
    const textErrors = textCompilationInfo.messages.filter((message) => message.type === 'error')
    if (textErrors.length > 0) {
      const message = textErrors.map((error) => error.message).join(' | ')
      throw new Error(message)
    }

    const textPipeline = device.createRenderPipeline({
      layout: 'auto',
      vertex: {
        module: textShaderModule,
        entryPoint: 'vs_main',
      },
      fragment: {
        module: textShaderModule,
        entryPoint: 'fs_main',
        targets: [
          {
            format: presentationFormat,
            blend: {
              color: {
                srcFactor: 'src-alpha',
                dstFactor: 'one-minus-src-alpha',
                operation: 'add',
              },
              alpha: {
                srcFactor: 'one',
                dstFactor: 'one-minus-src-alpha',
                operation: 'add',
              },
            },
          },
        ],
      },
      primitive: {
        topology: 'triangle-list',
      },
    })

    const makeTextBuffers = (
      layout: TextLayout,
      options?: { glyphBuffer?: GPUBuffer; glyphCount?: number }
    ) => {
      let glyphBuffer = options?.glyphBuffer
      let glyphCount = options?.glyphCount ?? layout.text.length
      if (!glyphBuffer) {
        const codes = new Uint32Array(Array.from(layout.text).map((char) => char.charCodeAt(0)))
        glyphBuffer = device.createBuffer({
          size: codes.byteLength,
          usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        })
        device.queue.writeBuffer(glyphBuffer, 0, codes)
        glyphCount = codes.length
      }

      const uniformData = new Float32Array([
        CANVAS_WIDTH,
        CANVAS_HEIGHT,
        layout.x,
        layout.y,
        FONT_GLYPH_SIZE,
        FONT_GLYPH_SIZE,
        FONT_ATLAS_WIDTH,
        FONT_ATLAS_HEIGHT,
        layout.color[0],
        layout.color[1],
        layout.color[2],
        layout.color[3],
      ])
      const uniformBuffer = device.createBuffer({
        size: uniformData.byteLength,
        usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
      })
      device.queue.writeBuffer(uniformBuffer, 0, uniformData)

      const textBindGroup = device.createBindGroup({
        layout: textPipeline.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: { buffer: uniformBuffer } },
          { binding: 1, resource: { buffer: glyphBuffer } },
          { binding: 2, resource: fontSampler },
          { binding: 3, resource: fontTexture.createView() },
        ],
      })

      return { textBindGroup, glyphCount }
    }

    const gateText = makeTextBuffers(textLayout.gateLabel)
    const stateTextDraw = makeTextBuffers(textLayout.stateVector, {
      glyphBuffer: stateTextGlyphBuffer,
      glyphCount: textLayout.stateVector.glyphCount ?? STATE_TEXT_MAX_LEN,
    })

    const instanceCount = instances.length
    let hasCaptured = false
    const renderFrame = () => {
      const commandEncoder = device.createCommandEncoder()
      const currentTexture = context.getCurrentTexture()
      const pass = commandEncoder.beginRenderPass({
        colorAttachments: [
          {
            view: currentTexture.createView(),
            clearValue: { r: COLORS.background[0], g: COLORS.background[1], b: COLORS.background[2], a: 1 },
            loadOp: 'clear',
            storeOp: 'store',
          },
        ],
      })
      pass.setPipeline(pipeline)
      pass.setBindGroup(0, bindGroup)
      pass.setVertexBuffer(0, instanceBuffer)
      pass.draw(6, instanceCount, 0, 0)
      pass.setPipeline(textPipeline)
      pass.setBindGroup(0, gateText.textBindGroup)
      pass.draw(6, gateText.glyphCount, 0, 0)
      pass.setBindGroup(0, stateTextDraw.textBindGroup)
      pass.draw(6, stateTextDraw.glyphCount, 0, 0)
      pass.end()
      if (!hasCaptured) {
        device.queue.submit([commandEncoder.finish()])
        requestAnimationFrame(() => {
          window.__renderDone = true
        })

        hasCaptured = true
      } else {
        device.queue.submit([commandEncoder.finish()])
      }
      requestAnimationFrame(renderFrame)
    }
    requestAnimationFrame(renderFrame)
  } catch (error) {
    const message =
      error && typeof (error as { message?: unknown }).message === 'string'
        ? (error as { message: string }).message
        : String(error)
    setStatus(message)
    window.__renderDone = true
    throw error
  }
}

window.__renderDone = false
window.__debugPixel = undefined
if (window.__captureFrame === undefined) {
  window.__captureFrame = false
}
if (window.__captureStateVector === undefined) {
  window.__captureStateVector = false
}
window.__frameDataUrl = undefined
window.__stateVector = undefined
init().catch((error) => {
  console.error(error)
})
