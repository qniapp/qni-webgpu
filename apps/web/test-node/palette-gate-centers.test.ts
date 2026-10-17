export {}

const { test } = require('node:test')
const assert = require('node:assert/strict')
const { getPaletteGateCenter } = require('../features/support/egui-helpers')
const { buildDragOperation } = require('../test-support/agent-visual-command')

// Flat gate IDs stay stable when a new entry is inserted into a palette row.
for (const { name, index, x } of [
  { name: 'ZParityControl', index: 31, x: 159 },
  { name: 'XParityControl', index: 32, x: 207 },
  { name: 'ZDetectControlReset', index: 26, x: 399 },
  { name: 'XDetector', index: 34, x: 447 },
  { name: 'YDetector', index: 35, x: 495 },
  { name: 'QFT', index: 22, x: 543 },
  { name: 'QFT†', index: 23, x: 591 },
  { name: 'Spacer', index: 21, x: 639 },
  { name: 'postselect0', index: 27, x: 687 },
  { name: 'postselect1', index: 28, x: 735 },
  { name: 'postselect-plus', index: 29, x: 783 },
  { name: 'postselect-minus', index: 30, x: 831 },
]) {
  test(`palette helper locates ${name} after Measurement`, () => {
    assert.deepEqual(getPaletteGateCenter(1000, index), { x, y: 148 })
  })

  test(`semantic drag helper locates ${name} after Measurement`, () => {
    assert.deepEqual(buildDragOperation({ cssWidth: 1000, gate: name, wire: 'q0', slot: 1 }).from,
      { x, y: 148 })
  })
}

for (const [cssWidth, x] of [[1001, 15], [1025, 27]]) {
  test(`semantic drag helper matches Rust rounding at width ${cssWidth}`, () => {
    assert.deepEqual(buildDragOperation({ cssWidth, gate: 'H', wire: 'q0', slot: 1 }).from,
      { x, y: 100 })
  })
}

test('semantic drag helper locates Spacer after both QFT gates', () => {
  assert.deepEqual(buildDragOperation({ cssWidth: 1000, gate: 'spacer', wire: 'q0', slot: 1 }).from,
    { x: 639, y: 148 })
})
