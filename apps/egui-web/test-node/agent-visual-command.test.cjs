const test = require('node:test')
const assert = require('node:assert/strict')
require('ts-node/register/transpile-only')

const {
  buildDragOperation,
  buildScreenshotPlan,
  getGateIndex,
  parseWire,
  parseOperations,
} = require('../test-support/agent-visual-command.ts')

test('agent visual command resolves palette gate aliases', () => {
  assert.equal(getGateIndex('H'), 0)
  assert.equal(getGateIndex('control'), 1)
  assert.equal(getGateIndex('sqrtx'), 5)
  assert.equal(getGateIndex('s†'), 7)
  assert.equal(getGateIndex('tdagger'), 9)
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
    gateIndex: 2,
    wire: 1,
    slot: 2,
    from: { x: 260, y: 80 },
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
