const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

const rootDir = path.join(__dirname, '..')
const shaderPath = path.join(rootDir, 'src', 'gpu', 'shaders', 'probability.rs')
const callbackPath = path.join(rootDir, 'src', 'gpu', 'callbacks', 'probability_display.rs')

const readProbabilityShader = async () => fs.readFile(shaderPath, 'utf8')
const readRenderShader = async () => {
  const shader = await readProbabilityShader()
  return shader.split('pub(in crate::gpu) const PROBABILITY_RENDER_SHADER')[1] ?? ''
}

test('dense Probability rendering defines a GPU preaggregation shader', async () => {
  const shader = await readProbabilityShader()

  assert.equal(shader.includes('const PROBABILITY_AGGREGATE_SHADER'), true)
})

test('dense Probability rendering samples the aggregate buffer', async () => {
  const renderShader = await readRenderShader()

  assert.equal(renderShader.includes('probability_aggregate_data['), true)
})

test('dense Probability rendering dispatches the aggregate pass', async () => {
  const callback = await fs.readFile(callbackPath, 'utf8')

  assert.equal(callback.includes('probability_aggregate_rows_pass'), true)
})

test('Probability hover outline snaps fractional rows to a pixel-aligned box', async () => {
  const renderShader = await readRenderShader()

  assert.match(renderShader, /fn pixel_row_top[\s\S]*let row_top = pixel_row_top\(hovered_row, row_h, rect_size\.y\)[\s\S]*local\.y < row_top/)
})

test('dense Probability rendering avoids fragment-row scans', async () => {
  const renderShader = await readRenderShader()

  assert.equal(/var row = row_lo;[\s\S]*?row = row \+ 1u;/.test(renderShader), false)
})

export {}
