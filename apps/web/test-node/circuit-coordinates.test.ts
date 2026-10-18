const test = require('node:test')
const assert = require('node:assert/strict')
const { fullPaletteCircuitOffsetY } = require('../test-support/palette-layout')
const { getCircuitWireY, getCircuitCellCenter, getCircuitOriginY } = require('../features/support/egui-helpers')
const { UI_CONSTANTS } = require('../test-support/generated-ui-constants')

for (const [width, offset] of [[1000, 48], [1280, 0]]) {
  test(`full palette circuit offset at ${width}px`, () => {
    assert.equal(fullPaletteCircuitOffsetY(width), offset)
  })
  test(`circuit wire center follows the full palette at ${width}px`, () => {
    assert.equal(getCircuitWireY(width, 1), UI_CONSTANTS.LINE_Y + offset + UI_CONSTANTS.LINE_GAP)
  })
  test(`circuit origin uses the actual canvas width at ${width}px`, async () => {
    const page = { locator: () => ({ boundingBox: async () => ({ width }) }) }
    assert.equal(await getCircuitOriginY(page), UI_CONSTANTS.LINE_Y + offset)
  })
  test(`circuit cell center follows the full palette at ${width}px`, () => {
    assert.deepEqual(getCircuitCellCenter(width, 2, 1), {
      x: 8 + UI_CONSTANTS.LINE_LEFT_OFFSET + UI_CONSTANTS.GATE_SIZE + 2 * UI_CONSTANTS.SLOT_SPACING,
      y: 8 + UI_CONSTANTS.LINE_Y + offset + UI_CONSTANTS.LINE_GAP,
    })
  })
}

export {}
