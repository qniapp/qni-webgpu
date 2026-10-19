export {}

const { test } = require('node:test')
const assert = require('node:assert/strict')
const { getPaletteGateCenter } = require('../features/support/egui-helpers')
const { buildDragOperation } = require('../test-support/agent-visual-command')
const { UI_CONSTANTS } = require('../test-support/generated-ui-constants')

// Flat gate IDs stay stable when a new entry is inserted into a palette row.
for (const { name, index, x, y, row } of [
  { name: 'ZParityControl', index: 31, x: 207, y: 148, row: 2 },
  { name: 'XParityControl', index: 32, x: 255, y: 148, row: 2 },
  { name: 'ZDetectControlReset', index: 26, x: 447, y: 148, row: 2 },
  { name: 'XDetector', index: 34, x: 63, y: 196, row: 3 },
  { name: 'YDetector', index: 35, x: 111, y: 196, row: 3 },
  { name: 'QFT', index: 22, x: 159, y: 196, row: 3 },
  { name: 'QFT†', index: 23, x: 207, y: 196, row: 3 },
  { name: 'Spacer', index: 21, x: 841, y: 148, row: 2 },
  { name: 'postselect0', index: 27, x: 255, y: 196, row: 3 },
  { name: 'postselect1', index: 28, x: 303, y: 196, row: 3 },
  { name: 'postselect-plus', index: 29, x: 351, y: 196, row: 3 },
  { name: 'postselect-minus', index: 30, x: 399, y: 196, row: 3 },
]) {
  test(`palette helper locates ${name} on row ${row} at 1000 px`, () => {
    assert.deepEqual(getPaletteGateCenter(1000, index), { x, y })
  })

  test(`semantic drag helper locates ${name} on row ${row} at 1000 px`, () => {
    assert.deepEqual(buildDragOperation({ cssWidth: 1000, gate: name, wire: 'q0', slot: 1 }).from,
      { x, y })
  })
}

for (const [cssWidth, x] of [[1001, 64], [1025, 76]]) {
  test(`semantic drag helper matches Rust rounding at width ${cssWidth}`, () => {
    assert.deepEqual(buildDragOperation({ cssWidth, gate: 'H', wire: 'q0', slot: 1 }).from,
      { x, y: 100 })
  })
}

test('semantic drag helper locates Spacer immediately left of the comment block', () => {
  assert.deepEqual(buildDragOperation({ cssWidth: 1000, gate: 'spacer', wire: 'q0', slot: 1 }).from,
    { x: 841, y: 148 })
})

test('palette helper keeps gate 7 at the end of the first wrapped row at 800 px', () => {
  assert.deepEqual(getPaletteGateCenter(800, 7), { x: 467, y: 100 })
})

test('palette helper keeps the right section beside the wrapped first gate row', () => {
  assert.deepEqual(getPaletteGateCenter(800, 25), { x: 525, y: 148 })
})

test('semantic drag helper moves the drop target below a four-row palette', () => {
  assert.equal(buildDragOperation({ cssWidth: 800, gate: 'H', wire: 'q0', slot: 0 }).to.y, UI_CONSTANTS.LINE_Y + 96)
})

test('semantic drag helper moves the drop target below a four-row palette at 844 px', () => {
  assert.equal(buildDragOperation({ cssWidth: 844, gate: 'H', wire: 'q0', slot: 0 }).to.y, UI_CONSTANTS.LINE_Y + 96)
})

test('palette helper locates the comment block right after Spacer in the right section', () => {
  assert.deepEqual(getPaletteGateCenter(1000, 38), { x: 889, y: 148 })
})

// 表示と、見た目のための指示（注釈や空きマス）は右側に置く。
for (const [index, x, y] of [
  [16, 981, 100], [20, 1029, 100], [33, 1077, 100], [24, 1125, 100],
  [25, 981, 148], [21, 1029, 148], [38, 1077, 148],
]) {
  test(`wide palette places right-side item ${index} at its concrete position`, () => {
    assert.deepEqual(getPaletteGateCenter(1280, index), { x, y })
  })
}

test('Spacer is immediately left of the comment block', () => {
  const comment = getPaletteGateCenter(1280, 38)
  assert.deepEqual(getPaletteGateCenter(1280, 21), { x: comment.x - 48, y: comment.y })
})

test('Spacer is immediately right of Density without an empty slot between them', () => {
  const density = getPaletteGateCenter(1280, 25)
  assert.deepEqual(getPaletteGateCenter(1280, 21), { x: density.x + 48, y: density.y })
})

for (const [cssWidth, contentWidth, contentHeight, panelWidth, panelHeight] of [
  [1280, 1010, 88, 1042, 128],
  [1000, 914, 136, 946, 176],
]) {
  test(`palette content and panel dimensions at ${cssWidth} px`, () => {
    const centers = Array.from({ length: 39 }, (_, index) => getPaletteGateCenter(cssWidth, index))
    const width = Math.max(...centers.map(p => p.x)) - Math.min(...centers.map(p => p.x)) + 40
    const height = Math.max(...centers.map(p => p.y)) - Math.min(...centers.map(p => p.y)) + 40
    assert.deepEqual({ contentWidth: width, contentHeight: height, panelWidth: width + 32, panelHeight: height + 40 },
      { contentWidth, contentHeight, panelWidth, panelHeight })
  })
}

test('wide palette keeps the first gates and arithmetic positions unchanged', () => {
  assert.deepEqual([0, 37, 13, 26].map(index => getPaletteGateCenter(1280, index)), [
    { x: 155, y: 100 }, { x: 827, y: 100 }, { x: 155, y: 148 }, { x: 539, y: 148 },
  ])
})

// 表示と、見た目のための指示（注釈や空きマス）の各形状を確認する。
for (const [cssWidth, positions] of [
  [480, [[317, 100], [365, 100], [413, 100], [317, 148], [365, 148], [317, 196], [365, 196]]],
  [640, [[517, 100], [565, 100], [517, 148], [565, 148], [517, 196], [517, 244], [565, 244]]],
] as const) {
  test(`right-side items use the expected grid at ${cssWidth} px`, () => {
    assert.deepEqual([16, 20, 33, 24, 25, 21, 38].map(index => getPaletteGateCenter(cssWidth, index)),
      positions.map(([x, y]) => ({ x, y })))
  })
}

test('palette chooses a shorter two-column layout over an earlier taller layout at 640 px', () => {
  assert.equal(buildDragOperation({ cssWidth: 640, gate: 'H', wire: 'q0', slot: 0 }).to.y,
    UI_CONSTANTS.LINE_Y + 96)
})
