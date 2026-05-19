const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

const rootDir = path.join(__dirname, '..')
const shaderPath = path.join(rootDir, 'src', 'gpu', 'shaders', 'chance.rs')
const callbackPath = path.join(rootDir, 'src', 'gpu', 'callbacks', 'chance_display.rs')

const readChanceShader = async () => fs.readFile(shaderPath, 'utf8')
const readRenderShader = async () => {
  const shader = await readChanceShader()
  return shader.split('pub(in crate::gpu) const CHANCE_RENDER_SHADER')[1] ?? ''
}

test('dense Chance rendering defines a GPU preaggregation shader', async () => {
  const shader = await readChanceShader()

  assert.equal(shader.includes('const CHANCE_AGGREGATE_SHADER'), true)
})

test('dense Chance rendering samples the aggregate buffer', async () => {
  const renderShader = await readRenderShader()

  assert.equal(renderShader.includes('chance_aggregate_data['), true)
})

test('dense Chance rendering dispatches the aggregate pass', async () => {
  const callback = await fs.readFile(callbackPath, 'utf8')

  assert.equal(callback.includes('chance_aggregate_rows_pass'), true)
})

test('Chance hover outline snaps fractional rows to a pixel-aligned box', async () => {
  const renderShader = await readRenderShader()

  assert.match(renderShader, /let row_top = clamp\(floor\(raw_row_top \+ 0\.5\)[\s\S]*let row_bottom = clamp\(floor\(raw_row_bottom \+ 0\.5\)[\s\S]*local\.y < row_top/)
})

test('dense Chance rendering avoids fragment-row scans', async () => {
  const renderShader = await readRenderShader()

  assert.equal(/var row = row_lo;[\s\S]*?row = row \+ 1u;/.test(renderShader), false)
})

export {}
