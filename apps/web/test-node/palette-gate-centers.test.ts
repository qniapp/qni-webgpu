export {}

const { test } = require('node:test')
const assert = require('node:assert/strict')
const { getPaletteGateCenter } = require('../features/support/egui-helpers')
const { buildDragOperation } = require('../test-support/agent-visual-command')

// Flat gate IDs stay stable when a new entry is inserted into a palette row.
for (const { name, index, x } of [
  { name: 'ZDetectControlReset', index: 26, x: 423 },
  { name: 'QFT', index: 22, x: 471 },
  { name: 'QFT†', index: 23, x: 519 },
  { name: 'Spacer', index: 21, x: 567 },
  { name: 'postselect0', index: 27, x: 615 },
  { name: 'postselect1', index: 28, x: 663 },
  { name: 'postselect-plus', index: 29, x: 711 },
  { name: 'postselect-minus', index: 30, x: 759 },
]) {
  test(`palette helper locates ${name} after Measurement`, () => {
    assert.deepEqual(getPaletteGateCenter(1000, index), { x, y: 148 })
  })

  test(`semantic drag helper locates ${name} after Measurement`, () => {
    assert.deepEqual(buildDragOperation({ cssWidth: 1000, gate: name, wire: 'q0', slot: 1 }).from,
      { x, y: 148 })
  })
}

test('semantic drag helper locates Spacer after both QFT gates', () => {
  assert.deepEqual(buildDragOperation({ cssWidth: 1000, gate: 'spacer', wire: 'q0', slot: 1 }).from,
    { x: 567, y: 148 })
})
