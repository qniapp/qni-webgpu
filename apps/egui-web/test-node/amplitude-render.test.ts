const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

const rootDir = path.join(__dirname, '..')
const shaderPath = path.join(rootDir, 'src', 'gpu', 'shaders', 'amplitude.rs')
const circuitGatesPath = path.join(rootDir, 'src', 'render', 'circuit_gates.rs')

const readRenderShader = async () => {
  const shader = await fs.readFile(shaderPath, 'utf8')
  return shader.split('pub(in crate::gpu) const AMPLITUDE_RENDER_SHADER')[1] ?? ''
}

test('Amplitude rendering anti-aliases circle SDF edges with derivatives', async () => {
  const renderShader = await readRenderShader()

  assert.match(renderShader, /let aa_edge = length\(fwidth\(in\.local\)\);[\s\S]*let edge = max\(0\.5, aa_edge \* 0\.65\);[\s\S]*smoothstep\(radius - edge, radius \+ edge, centered_len\)/)
})

test('Amplitude circle radius leaves slack inside the matrix frame', async () => {
  const renderShader = await readRenderShader()

  assert.match(renderShader, /let outline_radius = max\(0\.0, cell \* 0\.5 - stroke\);/)
})

test('Amplitude rendering does not draw internal grid separators', async () => {
  const renderShader = await readRenderShader()

  assert.equal(renderShader.includes('min(gx, gy)'), false)
})

test('Amplitude rendering treats tiny non-zero amplitudes as non-zero outlines', async () => {
  const renderShader = await readRenderShader()

  assert.match(renderShader, /select\(params\.outline_zero, params\.outline, mag > 0\.000001\)/)
})

test('Amplitude rendering uses purple background only for dragged instances', async () => {
  const renderShader = await readRenderShader()

  assert.match(renderShader, /if \(in\.use_drag_background == 1u\) \{\s*color = params\.drag_background;/)
})

test('Amplitude dragged background preserves the circle interior background', async () => {
  const renderShader = await readRenderShader()

  assert.match(renderShader, /var circle_background = params\.background;[\s\S]*color = blend_over\(color, circle_background\);/)
})

test('Amplitude rendering keeps outlines enabled for 15-qubit-sized cells', async () => {
  const renderShader = await readRenderShader()

  assert.match(renderShader, /if \(cell >= 3\.0\) \{[\s\S]*var outline = select\(params\.outline_zero, params\.outline, mag > 0\.000001\)/)
})

test('Amplitude circuit rendering uses the matrix draw area as the visible body', async () => {
  const circuitGates = await fs.readFile(circuitGatesPath, 'utf8')

  assert.match(circuitGates, /let body_rect = if gate\.kind == GateKind::AmplitudeDisplay \{[\s\S]*amplitude_grid_rect\(gate_rect, gate\.span\)[\s\S]*draw_gate_body\(painter, body_rect, gate\.kind, colors\)/)
})

export {}
