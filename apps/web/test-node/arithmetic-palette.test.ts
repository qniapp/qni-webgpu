import { test } from 'node:test'
import assert from 'node:assert/strict'
import { getPaletteGateCenter } from '../features/support/egui-helpers'
import { buildDragOperation, getGateIndex } from '../test-support/agent-visual-command'

for (const [id, col, token] of [[36, 13, 'inc'], [37, 14, 'dec']] as const) {
  test(`${token} palette helper uses free first-row space`, () => {
    assert.deepEqual(getPaletteGateCenter(1000, id), { x: 15 + 48 * col, y: 100 })
  })
  test(`${token} semantic drag agrees with palette helper`, () => {
    assert.deepEqual(buildDragOperation({ gate: token, wire: 'q0', slot: '0', cssWidth: 1000 }).from,
      { x: 15 + 48 * col, y: 100 })
  })
  test(`${token} resolves to its stable palette id`, () => {
    assert.equal(getGateIndex(token), id)
  })
}
