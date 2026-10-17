export {}

const { test } = require('node:test')
const assert = require('node:assert/strict')
const { getPaletteGateCenter } = require('../features/support/egui-helpers')
const { buildDragOperation } = require('../test-support/agent-visual-command')
const { UI_CONSTANTS } = require('../test-support/generated-ui-constants')

// Flat gate IDs stay stable when a new entry is inserted into a palette row.
for (const { name, index, x } of [
  { name: 'ZParityControl', index: 31, x: 207 },
  { name: 'XParityControl', index: 32, x: 255 },
  { name: 'ZDetectControlReset', index: 26, x: 447 },
  { name: 'XDetector', index: 34, x: 495 },
  { name: 'YDetector', index: 35, x: 543 },
  { name: 'QFT', index: 22, x: 591 },
  { name: 'QFT†', index: 23, x: 639 },
  { name: 'Spacer', index: 21, x: 687 },
  { name: 'postselect0', index: 27, x: 735 },
  { name: 'postselect1', index: 28, x: 783 },
  { name: 'postselect-plus', index: 29, x: 831 },
  { name: 'postselect-minus', index: 30, x: 879 },
]) {
  test(`palette helper locates ${name} after Measurement`, () => {
    assert.deepEqual(getPaletteGateCenter(1000, index), { x, y: 148 })
  })

  test(`semantic drag helper locates ${name} after Measurement`, () => {
    assert.deepEqual(buildDragOperation({ cssWidth: 1000, gate: name, wire: 'q0', slot: 1 }).from,
      { x, y: 148 })
  })
}

for (const [cssWidth, x] of [[1001, 64], [1025, 76]]) {
  test(`semantic drag helper matches Rust rounding at width ${cssWidth}`, () => {
    assert.deepEqual(buildDragOperation({ cssWidth, gate: 'H', wire: 'q0', slot: 1 }).from,
      { x, y: 100 })
  })
}

test('semantic drag helper locates Spacer after both QFT gates', () => {
  assert.deepEqual(buildDragOperation({ cssWidth: 1000, gate: 'spacer', wire: 'q0', slot: 1 }).from,
    { x: 687, y: 148 })
})

test('palette helper wraps the first gate row on an 800 px canvas', () => {
  assert.deepEqual(getPaletteGateCenter(800, 7), { x: 179, y: 148 })
})

test('palette helper keeps the Display section beside the wrapped first gate row', () => {
  assert.deepEqual(getPaletteGateCenter(800, 25), { x: 573, y: 148 })
})

test('semantic drag helper moves the drop target below a four-row palette', () => {
  assert.equal(buildDragOperation({ cssWidth: 800, gate: 'H', wire: 'q0', slot: 0 }).to.y, UI_CONSTANTS.LINE_Y + 96)
})

test('semantic drag helper moves the drop target below a three-row palette', () => {
  assert.equal(buildDragOperation({ cssWidth: 844, gate: 'H', wire: 'q0', slot: 0 }).to.y, UI_CONSTANTS.LINE_Y + 48)
})

test('palette helper locates the comment block in the Display section last slot', () => {
  assert.deepEqual(getPaletteGateCenter(1000, 36), { x: 937, y: 100 })
})
