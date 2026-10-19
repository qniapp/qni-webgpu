import { test } from 'node:test'
import assert from 'node:assert/strict'
import { getPaletteGateCenter } from '../features/support/egui-helpers'
import { buildDragOperation, getGateIndex } from '../test-support/agent-visual-command'

for (const [id, x, token] of [[36, 687, 'inc'], [37, 735, 'dec']] as const) {
  test(`${token} palette helper occupies the first row at 1000 px`, () => {
    assert.deepEqual(getPaletteGateCenter(1000, id), { x, y: 100 })
  })
  test(`${token} semantic drag starts on the first row at 1000 px`, () => {
    assert.deepEqual(buildDragOperation({ gate: token, wire: 'q0', slot: '0', cssWidth: 1000 }).from,
      { x, y: 100 })
  })
  test(`${token} resolves to its stable palette id`, () => {
    assert.equal(getGateIndex(token), id)
  })
}

// At 800 px the first group wraps 8/7; at 1280 px all 15 entries fit one row.
for (const [cssWidth, id, token, x, y, row] of [
  [800, 36, 'inc', 371, 148, 2],
  [800, 37, 'dec', 419, 148, 2],
  [1280, 36, 'inc', 779, 100, 1],
  [1280, 37, 'dec', 827, 100, 1],
] as const) {
  test(`${token} palette helper occupies row ${row} at ${cssWidth} px`, () => {
    assert.deepEqual(getPaletteGateCenter(cssWidth, id), { x, y })
  })
  test(`${token} semantic drag starts on row ${row} at ${cssWidth} px`, () => {
    assert.deepEqual(buildDragOperation({ gate: token, wire: 'q0', slot: '0', cssWidth }).from,
      { x, y })
  })
}
