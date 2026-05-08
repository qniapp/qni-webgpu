const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

const {
  buildDragOperation,
  buildScreenshotPlan,
  getGateIndex,
  parseWire,
  parseOperations,
} = require('../test-support/agent-visual-command.ts')

const rootDir = path.join(__dirname, '..')
const repoRoot = path.join(rootDir, '..', '..')
const readText = (filePath: string) => fs.readFile(filePath, 'utf8')

test('agent visual command resolves palette gate aliases', () => {
  assert.equal(getGateIndex('H'), 0)
  assert.equal(getGateIndex('X'), 1)
  assert.equal(getGateIndex('sqrtx'), 4)
  assert.equal(getGateIndex('s†'), 6)
  assert.equal(getGateIndex('tdagger'), 8)
  assert.equal(getGateIndex('swap'), 13)
  assert.equal(getGateIndex('control'), 14)
  assert.equal(getGateIndex('anti-control'), 15)
  assert.equal(getGateIndex('◦'), 15)
  assert.equal(getGateIndex('bloch'), 16)
  assert.equal(getGateIndex('sphere'), 16)
  assert.equal(getGateIndex('|0>'), 17)
  assert.equal(getGateIndex('|0⟩'), 17)
  assert.equal(getGateIndex('write0'), 17)
  assert.equal(getGateIndex('|1>'), 18)
  assert.equal(getGateIndex('write1'), 18)
})

test('agent visual command parses q-prefixed and numeric wires', () => {
  assert.equal(parseWire('q0'), 0)
  assert.equal(parseWire('1'), 1)
})

test('agent visual command builds drag coordinates from semantic gate placement', () => {
  const operation = buildDragOperation({
    cssWidth: 1000,
    gate: 'x',
    wire: 'q1',
    slot: 2,
  })

  assert.deepEqual(operation, {
    gate: 'x',
    gateIndex: 1,
    wire: 1,
    slot: 2,
    from: { x: 300, y: 80 },
    to: { x: 250, y: 256 },
  })
})

test('agent visual command supports egui content vertical offset', () => {
  const operation = buildDragOperation({
    cssWidth: 1000,
    gate: 'h',
    wire: 'q0',
    slot: 0,
    verticalOffset: 8,
  })

  assert.equal(operation.from.y, 88)
  assert.equal(operation.to.y, 216)
})

test('agent visual command parses comma separated operations', () => {
  assert.deepEqual(parseOperations('H:q0:0,C:q0:1,X:q1:1'), [
    { gate: 'H', wire: 'q0', slot: 0 },
    { gate: 'C', wire: 'q0', slot: 1 },
    { gate: 'X', wire: 'q1', slot: 1 },
  ])
})

test('agent visual command writes page screenshots by default', () => {
  assert.deepEqual(buildScreenshotPlan({ command: 'drag' }), {
    pageOut: 'output/playwright/agent-visual/drag.png',
    canvasOut: null,
  })

  assert.deepEqual(buildScreenshotPlan({
    command: 'drag',
    out: 'page.png',
    canvasOut: 'canvas.png',
  }), {
    pageOut: 'page.png',
    canvasOut: 'canvas.png',
  })
})

test('agent visual CLI uses a generic name without a compatibility wrapper', async () => {
  await assert.doesNotReject(() => fs.access(path.join(rootDir, 'scripts', 'agent-visual.ts')))
  await assert.rejects(() => fs.access(path.join(rootDir, 'scripts', 'agent-visual.cjs')), /ENOENT/)
  await assert.rejects(() => fs.access(path.join(rootDir, 'scripts', 'codex-visual.cjs')), /ENOENT/)

  const docs = await readText(path.join(repoRoot, 'docs', 'egui-web.md'))
  assert.match(docs, /scripts\/agent-visual\.ts/)
  assert.doesNotMatch(docs, /scripts\/agent-visual\.cjs/)

  const agents = await readText(path.join(repoRoot, 'AGENTS.md'))
  assert.match(agents, /後方互換/)
  assert.match(agents, /残さない/)
})

export {}
