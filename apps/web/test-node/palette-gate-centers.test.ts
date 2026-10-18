export {}

const { test } = require('node:test')
const assert = require('node:assert/strict')
const { getPaletteGateCenter } = require('../features/support/egui-helpers')
const { buildDragOperation } = require('../test-support/agent-visual-command')
const { UI_CONSTANTS } = require('../test-support/generated-ui-constants')

// Flat gate IDs stay stable when a new entry is inserted into a palette row.
for (const { name, index, x, y, row } of [
  { name: 'ZParityControl', index: 31, x: 231, y: 148, row: 2 },
  { name: 'XParityControl', index: 32, x: 279, y: 148, row: 2 },
  { name: 'ZDetectControlReset', index: 26, x: 471, y: 148, row: 2 },
  { name: 'XDetector', index: 34, x: 87, y: 196, row: 3 },
  { name: 'YDetector', index: 35, x: 135, y: 196, row: 3 },
  { name: 'QFT', index: 22, x: 183, y: 196, row: 3 },
  { name: 'QFT†', index: 23, x: 231, y: 196, row: 3 },
  { name: 'Spacer', index: 21, x: 279, y: 196, row: 3 },
  { name: 'postselect0', index: 27, x: 327, y: 196, row: 3 },
  { name: 'postselect1', index: 28, x: 375, y: 196, row: 3 },
  { name: 'postselect-plus', index: 29, x: 423, y: 196, row: 3 },
  { name: 'postselect-minus', index: 30, x: 471, y: 196, row: 3 },
]) {
  test(`palette helper locates ${name} on row ${row} at 1000 px`, () => {
    assert.deepEqual(getPaletteGateCenter(1000, index), { x, y })
  })

  test(`semantic drag helper locates ${name} on row ${row} at 1000 px`, () => {
    assert.deepEqual(buildDragOperation({ cssWidth: 1000, gate: name, wire: 'q0', slot: 1 }).from,
      { x, y })
  })
}

for (const [cssWidth, x] of [[1001, 88], [1025, 100]]) {
  test(`semantic drag helper matches Rust rounding at width ${cssWidth}`, () => {
    assert.deepEqual(buildDragOperation({ cssWidth, gate: 'H', wire: 'q0', slot: 1 }).from,
      { x, y: 100 })
  })
}

test('semantic drag helper locates Spacer after both QFT gates', () => {
  assert.deepEqual(buildDragOperation({ cssWidth: 1000, gate: 'spacer', wire: 'q0', slot: 1 }).from,
    { x: 279, y: 196 })
})

test('palette helper keeps gate 7 at the end of the first wrapped row at 800 px', () => {
  assert.deepEqual(getPaletteGateCenter(800, 7), { x: 491, y: 100 })
})

test('palette helper keeps the Display section beside the wrapped first gate row', () => {
  assert.deepEqual(getPaletteGateCenter(800, 25), { x: 597, y: 148 })
})

test('semantic drag helper moves the drop target below a four-row palette', () => {
  assert.equal(buildDragOperation({ cssWidth: 800, gate: 'H', wire: 'q0', slot: 0 }).to.y, UI_CONSTANTS.LINE_Y + 96)
})

test('semantic drag helper moves the drop target below a four-row palette at 844 px', () => {
  assert.equal(buildDragOperation({ cssWidth: 844, gate: 'H', wire: 'q0', slot: 0 }).to.y, UI_CONSTANTS.LINE_Y + 96)
})

test('palette helper locates the comment block in the Display section last slot', () => {
  assert.deepEqual(getPaletteGateCenter(1000, 38), { x: 913, y: 148 })
})
