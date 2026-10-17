export {}

const { test } = require('node:test')
const assert = require('node:assert/strict')
const { getPaletteGateCenter } = require('../features/support/egui-helpers')
const { buildDragOperation } = require('../test-support/agent-visual-command')

// Flat gate IDs stay stable when a new entry is inserted into a palette row.
for (const { name, index, x } of [
  { name: 'ZParityControl', index: 27, x: 255 },
  { name: 'XParityControl', index: 28, x: 303 },
  { name: 'ZDetectControlReset', index: 26, x: 495 },
  { name: 'XDetector', index: 30, x: 543 },
  { name: 'YDetector', index: 31, x: 591 },
  { name: 'QFT', index: 22, x: 639 },
  { name: 'QFT†', index: 23, x: 687 },
  { name: 'Spacer', index: 21, x: 735 },
]) {
  test(`palette helper locates ${name} after Measurement`, () => {
    assert.deepEqual(getPaletteGateCenter(1000, index), { x, y: 148 })
  })
}

test('semantic drag helper locates Spacer after both QFT gates', () => {
  assert.deepEqual(buildDragOperation({ cssWidth: 1000, gate: 'spacer', wire: 'q0', slot: 1 }).from,
    { x: 735, y: 148 })
})

test('palette helper locates the comment block in the Display section bottom-right slot', () => {
  assert.deepEqual(getPaletteGateCenter(1000, 32), { x: 889, y: 148 })
})
