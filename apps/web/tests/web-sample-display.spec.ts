import { expect, test, type Page } from '@playwright/test'
import {
  getCircuitOriginY,
  pixelRgbDistance,
  pollForValue,
  readProbabilityDistributions,
  readStateVector,
  sampleCanvasPixels,
  UI_CONSTANTS,
  waitForStartupReady,
  waitForValue,
} from './support/web-spec-helpers'

type Column = (string | number)[]
const PANEL_MARGIN = 8
const LIT_CORE: [number, number, number, number] = [198, 221, 232, 255]
const UNLIT_FILL: [number, number, number, number] = [218, 216, 206, 255]
const BACKGROUND: [number, number, number, number] = [242, 240, 229, 255]
const PIXEL_TOLERANCE = 8
// The halo two pixels outside a lit tile is clearly blue, far from the background.
const HALO_MIN_DISTANCE = 48
const SPAN_RESIZE_HANDLE: [number, number, number, number] = [58, 169, 159, 255]
const SPAN_RESIZE_HANDLE_TOLERANCE = 56
const SAMPLE_X = PANEL_MARGIN + UI_CONSTANTS.LINE_LEFT_OFFSET + UI_CONSTANTS.GATE_SIZE + UI_CONSTANTS.SLOT_SPACING

const readCols = (page: Page): string => JSON.stringify(JSON.parse(decodeURIComponent(new URL(page.url()).hash.slice(1))).cols)

const dragSpanHandle = async (page: Page, handleY: number, wireDelta: number) => {
  await page.mouse.move(SAMPLE_X, handleY)
  await page.mouse.down()
  await page.mouse.move(SAMPLE_X, handleY + wireDelta * UI_CONSTANTS.LINE_GAP, { steps: 8 })
  await page.mouse.up()
}

const colsAfterChange = async (page: Page, before: string): Promise<string> =>
  pollForValue(() => Promise.resolve(readCols(page)), (cols) => cols !== before)

const openCircuit = async (page: Page, cols: Column[]) => {
  await page.goto(`/#${encodeURIComponent(JSON.stringify({ cols }))}`)
  await waitForStartupReady(page, { waitForStateVector: true })
  await page.mouse.move(900, 700)
}

const sampleOutcome = async (page: Page, span: number): Promise<number> => {
  const entries = await waitForValue(
    () => readProbabilityDistributions(page),
    (entries) => entries.length > 0,
    'Sample capture unavailable',
  )
  return page.evaluate(async ({ gateId, span }) => {
    const module = window.wasmBindings as unknown as {
      read_sample_display_outcome: (gateId: number, span: number) => Promise<number>
    }
    return module.read_sample_display_outcome(gateId, span)
  }, { gateId: entries[0].gateId, span })
}

for (const span of [1, 2, 3]) {
  for (let outcome = 0; outcome < 2 ** span; outcome++) {
    test(`Sample${span} samples basis state ${outcome}`, async ({ page }) => {
      await openCircuit(page, [
        Array.from({ length: span }, (_, wire) => `|${(outcome >> wire) & 1}>`),
        [`Sample${span}`],
      ])
      expect(await sampleOutcome(page, span)).toBe(outcome)
    })
  }

  test(`Sample${span} lights the tiles of on bits`, async ({ page }) => {
    const bits = Array.from({ length: span }, (_, wire) => wire % 2 === 0 ? 1 : 0)
    await openCircuit(page, [bits.map((bit) => `|${bit}>`), [`Sample${span}`]])
    await sampleOutcome(page, span)
    const originY = PANEL_MARGIN + await getCircuitOriginY(page)
    const wireY = (wire: number) => originY + wire * UI_CONSTANTS.LINE_GAP
    const pixels = await sampleCanvasPixels(page, page.locator('#egui-canvas'), bits.map((_, wire) => (
      { name: `tile${wire}`, x: SAMPLE_X, y: wireY(wire) }
    )))
    expect(bits.every((bit, wire) =>
      pixelRgbDistance(pixels[`tile${wire}`], bit ? LIT_CORE : UNLIT_FILL) < PIXEL_TOLERANCE,
    )).toBe(true)
  })
}

const SAMPLE8_BITS = [1, 0, 1, 0, 1, 1, 0, 1]
// One control fanning out to seven targets keeps Sample8 in column 2.
const GHZ8_SAMPLE8: Column[] = [['H'], ['•', 'X', 'X', 'X', 'X', 'X', 'X', 'X'], ['Sample8']]

for (const outcome of [0, 0b10110101, 255]) {
  test(`Sample8 samples basis state ${outcome}`, async ({ page }) => {
    await openCircuit(page, [
      Array.from({ length: 8 }, (_, wire) => `|${(outcome >> wire) & 1}>`),
      ['Sample8'],
    ])
    expect(await sampleOutcome(page, 8)).toBe(outcome)
  })
}

test('Sample8 on a GHZ state returns all zeros or all ones', async ({ page }) => {
  await openCircuit(page, GHZ8_SAMPLE8)
  expect([0, 255]).toContain(await sampleOutcome(page, 8))
})

// At the default 1000px width the state panel covers the lower Sample8 tiles.
test.describe('Sample8 at 1280x800', () => {
  test.use({ viewport: { width: 1280, height: 800 } })

  test('Sample8 lights the tiles of on bits', async ({ page }) => {
    await openCircuit(page, [SAMPLE8_BITS.map((bit) => `|${bit}>`), ['Sample8']])
    await sampleOutcome(page, 8)
    const originY = PANEL_MARGIN + await getCircuitOriginY(page)
    const wireY = (wire: number) => originY + wire * UI_CONSTANTS.LINE_GAP
    const pixels = await sampleCanvasPixels(page, page.locator('#egui-canvas'), SAMPLE8_BITS.map((_, wire) => (
      { name: `tile${wire}`, x: SAMPLE_X, y: wireY(wire) }
    )))
    expect(SAMPLE8_BITS.flatMap((bit, wire) =>
      pixelRgbDistance(pixels[`tile${wire}`], bit ? LIT_CORE : UNLIT_FILL) < PIXEL_TOLERANCE ? [] : [wire],
    )).toEqual([])
  })

  test('Sample8 tiles show the sampled outcome of a GHZ state', async ({ page }) => {
    await openCircuit(page, GHZ8_SAMPLE8)
    const outcome = await sampleOutcome(page, 8)
    const originY = PANEL_MARGIN + await getCircuitOriginY(page)
    const wireY = (wire: number) => originY + wire * UI_CONSTANTS.LINE_GAP
    const pixels = await sampleCanvasPixels(page, page.locator('#egui-canvas'), SAMPLE8_BITS.map((_, wire) => (
      { name: `tile${wire}`, x: SAMPLE_X + UI_CONSTANTS.SLOT_SPACING, y: wireY(wire) }
    )))
    expect(SAMPLE8_BITS.flatMap((_, wire) =>
      pixelRgbDistance(pixels[`tile${wire}`], (outcome >> wire) & 1 ? LIT_CORE : UNLIT_FILL) < PIXEL_TOLERANCE ? [] : [wire],
    )).toEqual([])
  })

  test('Sample8 visual snapshot', async ({ page }) => {
    await openCircuit(page, [SAMPLE8_BITS.map((bit) => `|${bit}>`), ['Sample8']])
    await page.mouse.move(1200, 780)
    await sampleOutcome(page, 8)
    await expect(page.locator('#egui-canvas')).toHaveScreenshot('sample8-10110101.png')
  })
})

test('Sample3 leaves the gaps between lit and unlit tiles transparent', async ({ page }) => {
  await openCircuit(page, [['X', 1, 'X'], ['Sample3']])
  await sampleOutcome(page, 3)
  const originY = PANEL_MARGIN + await getCircuitOriginY(page)
  const wireY = (wire: number) => originY + wire * UI_CONSTANTS.LINE_GAP
  const pixels = await sampleCanvasPixels(page, page.locator('#egui-canvas'), [0, 1].map((wire) => (
    { name: `gap${wire}`, x: SAMPLE_X, y: wireY(wire) + UI_CONSTANTS.LINE_GAP / 2 }
  )))
  expect([0, 1].every((wire) => pixelRgbDistance(pixels[`gap${wire}`], BACKGROUND) < PIXEL_TOLERANCE)).toBe(true)
})

test('Sample lit tile glows past its edge', async ({ page }) => {
  await openCircuit(page, [['X'], ['Sample1']])
  await sampleOutcome(page, 1)
  const originY = PANEL_MARGIN + await getCircuitOriginY(page)
  const wireY = (wire: number) => originY + wire * UI_CONSTANTS.LINE_GAP
  const pixels = await sampleCanvasPixels(page, page.locator('#egui-canvas'), [{
    name: 'halo', x: SAMPLE_X - UI_CONSTANTS.GATE_SIZE / 2 - 2, y: wireY(0) - 12,
  }])
  expect(pixelRgbDistance(pixels.halo, BACKGROUND)).toBeGreaterThan(HALO_MIN_DISTANCE)
})

test('Sample unlit tile has no halo', async ({ page }) => {
  await openCircuit(page, [['|0>'], ['Sample1']])
  await sampleOutcome(page, 1)
  const originY = PANEL_MARGIN + await getCircuitOriginY(page)
  const wireY = (wire: number) => originY + wire * UI_CONSTANTS.LINE_GAP
  const pixels = await sampleCanvasPixels(page, page.locator('#egui-canvas'), [{
    name: 'outside', x: SAMPLE_X - UI_CONSTANTS.GATE_SIZE / 2 - 2, y: wireY(0) - 12,
  }])
  expect(pixelRgbDistance(pixels.outside, BACKGROUND)).toBeLessThan(PIXEL_TOLERANCE)
})

test('Sample1 over zero shows an unlit grey tile', async ({ page }) => {
  await openCircuit(page, [['Sample1']])
  await sampleOutcome(page, 1)
  const centerX = PANEL_MARGIN + UI_CONSTANTS.LINE_LEFT_OFFSET + UI_CONSTANTS.GATE_SIZE
  const originY = PANEL_MARGIN + await getCircuitOriginY(page)
  const wireY = (wire: number) => originY + wire * UI_CONSTANTS.LINE_GAP
  const pixels = await sampleCanvasPixels(page, page.locator('#egui-canvas'), [
    { name: 'tile', x: centerX, y: wireY(0) },
  ])
  expect(pixelRgbDistance(pixels.tile, UNLIT_FILL)).toBeLessThan(PIXEL_TOLERANCE)
})

test('Sample3 101 visual snapshot', async ({ page }) => {
  await openCircuit(page, [['|1>', '|0>', '|1>'], ['Sample3']])
  await sampleOutcome(page, 3)
  await expect(page.locator('#egui-canvas')).toHaveScreenshot('sample3-101.png')
})

test('Sample free drag remains visible over the palette', async ({ page }) => {
  await openCircuit(page, [['X', 1, 'X'], ['Sample3']])
  await sampleOutcome(page, 3)
  const sourceX = PANEL_MARGIN + UI_CONSTANTS.LINE_LEFT_OFFSET + UI_CONSTANTS.GATE_SIZE + UI_CONSTANTS.SLOT_SPACING
  const originY = PANEL_MARGIN + await getCircuitOriginY(page)
  const wireY = (wire: number) => originY + wire * UI_CONSTANTS.LINE_GAP
  await page.mouse.move(sourceX, wireY(0))
  await page.mouse.down()
  await page.mouse.move(399, 108, { steps: 8 })
  const pixels = await sampleCanvasPixels(page, page.locator('#egui-canvas'), [
    { name: 'left', x: 383, y: 108 },
    { name: 'right', x: 415, y: 108 },
  ])
  expect(['left', 'right'].every((side) => pixelRgbDistance(pixels[side], UNLIT_FILL) < PIXEL_TOLERANCE)).toBe(true)
  await page.mouse.up()
})

test('Sample free drag visual snapshot', async ({ page }) => {
  await openCircuit(page, [['X', 1, 'X'], ['Sample3']])
  await sampleOutcome(page, 3)
  const sourceX = PANEL_MARGIN + UI_CONSTANTS.LINE_LEFT_OFFSET + UI_CONSTANTS.GATE_SIZE + UI_CONSTANTS.SLOT_SPACING
  const originY = PANEL_MARGIN + await getCircuitOriginY(page)
  const wireY = (wire: number) => originY + wire * UI_CONSTANTS.LINE_GAP
  await page.mouse.move(sourceX, wireY(0))
  await page.mouse.down()
  await page.mouse.move(399, 108, { steps: 8 })
  await expect(page.locator('#egui-canvas')).toHaveScreenshot('sample3-free-drag.png')
  await page.mouse.up()
})

test('Sample snapped drag shows the GPU sampled tile colors', async ({ page }) => {
  await openCircuit(page, [['X', 1, 'X'], ['Sample3']])
  await sampleOutcome(page, 3)
  const sourceX = PANEL_MARGIN + UI_CONSTANTS.LINE_LEFT_OFFSET + UI_CONSTANTS.GATE_SIZE + UI_CONSTANTS.SLOT_SPACING
  const targetX = sourceX + UI_CONSTANTS.SLOT_SPACING
  const originY = PANEL_MARGIN + await getCircuitOriginY(page)
  const wireY = (wire: number) => originY + wire * UI_CONSTANTS.LINE_GAP
  await page.mouse.move(sourceX, wireY(0))
  await page.mouse.down()
  await page.mouse.move(targetX, wireY(0), { steps: 8 })
  const pixels = await sampleCanvasPixels(page, page.locator('#egui-canvas'), [0, 1, 2].map((wire) => ({
    name: `wire${wire}`, x: targetX, y: wireY(wire),
  })))
  expect([0, 1, 2].every((wire) => pixelRgbDistance(pixels[`wire${wire}`], wire === 1 ? UNLIT_FILL : LIT_CORE) < PIXEL_TOLERANCE)).toBe(true)
  await page.mouse.up()
})

test('Sample resize from one to three wires lights the added tile', async ({ page }) => {
  await openCircuit(page, [['X', 1, 'X'], ['Sample1']])
  await sampleOutcome(page, 1)
  const centerX = PANEL_MARGIN + UI_CONSTANTS.LINE_LEFT_OFFSET + UI_CONSTANTS.GATE_SIZE + UI_CONSTANTS.SLOT_SPACING
  const originY = PANEL_MARGIN + await getCircuitOriginY(page)
  const wireY = (wire: number) => originY + wire * UI_CONSTANTS.LINE_GAP
  const handleY = wireY(0) + UI_CONSTANTS.GATE_SIZE / 2 + 9
  await page.mouse.move(centerX, handleY)
  await page.mouse.down()
  await page.mouse.move(centerX, handleY + 2 * UI_CONSTANTS.LINE_GAP, { steps: 8 })
  await page.mouse.up()
  await waitForValue(
    () => Promise.resolve(JSON.parse(decodeURIComponent(new URL(page.url()).hash.slice(1))).cols),
    (cols) => JSON.stringify(cols) === JSON.stringify([['X', 1, 'X'], ['Sample3']]),
    'Sample resize did not update the URL',
  )
  await sampleOutcome(page, 3)
  await page.mouse.move(900, 700)
  const pixels = await sampleCanvasPixels(page, page.locator('#egui-canvas'), [{
    name: 'added', x: centerX, y: wireY(2),
  }])
  expect(pixelRgbDistance(pixels.added, LIT_CORE)).toBeLessThan(PIXEL_TOLERANCE)
})

test('Sample bottom resize handle stretches Sample1 to Sample3', async ({ page }) => {
  await openCircuit(page, [['X', 1, 'X'], ['Sample1']])
  const before = readCols(page)
  const originY = PANEL_MARGIN + await getCircuitOriginY(page)
  const wireY = (wire: number) => originY + wire * UI_CONSTANTS.LINE_GAP
  await dragSpanHandle(page, wireY(0) + UI_CONSTANTS.GATE_SIZE / 2 + 9, 2)
  expect(await colsAfterChange(page, before)).toBe(JSON.stringify([['X', 1, 'X'], ['Sample3']]))
})

test('Sample top resize handle stretches Sample1 upward to Sample3', async ({ page }) => {
  await openCircuit(page, [['X', 1, 'X'], [1, 1, 'Sample1']])
  const before = readCols(page)
  const originY = PANEL_MARGIN + await getCircuitOriginY(page)
  const wireY = (wire: number) => originY + wire * UI_CONSTANTS.LINE_GAP
  await dragSpanHandle(page, wireY(2) - UI_CONSTANTS.GATE_SIZE / 2 - 9, -2)
  expect(await colsAfterChange(page, before)).toBe(JSON.stringify([['X', 1, 'X'], ['Sample3']]))
})

test('Sample bottom resize handle stretches Sample1 to Sample8', async ({ page }) => {
  await openCircuit(page, [['X', 1, 1, 1, 1, 1, 1, 'X'], ['Sample1']])
  const before = readCols(page)
  const originY = PANEL_MARGIN + await getCircuitOriginY(page)
  const wireY = (wire: number) => originY + wire * UI_CONSTANTS.LINE_GAP
  await dragSpanHandle(page, wireY(0) + UI_CONSTANTS.GATE_SIZE / 2 + 9, 7)
  expect(await colsAfterChange(page, before)).toBe(JSON.stringify([['X', 1, 1, 1, 1, 1, 1, 'X'], ['Sample8']]))
})

test('Sample resize stops at eight wires', async ({ page }) => {
  await openCircuit(page, [['X', 1, 1, 1, 1, 1, 1, 1, 1, 'X'], ['Sample1']])
  const before = readCols(page)
  const originY = PANEL_MARGIN + await getCircuitOriginY(page)
  const wireY = (wire: number) => originY + wire * UI_CONSTANTS.LINE_GAP
  await dragSpanHandle(page, wireY(0) + UI_CONSTANTS.GATE_SIZE / 2 + 9, 9)
  expect(await colsAfterChange(page, before)).toBe(JSON.stringify([['X', 1, 1, 1, 1, 1, 1, 1, 1, 'X'], ['Sample8']]))
})

test('Sample resize handles show two pills around the tile column', async ({ page }) => {
  await openCircuit(page, [['X', 1, 'X'], [1, 'Sample1']])
  const originY = PANEL_MARGIN + await getCircuitOriginY(page)
  const wireY = (wire: number) => originY + wire * UI_CONSTANTS.LINE_GAP
  await page.mouse.move(SAMPLE_X, wireY(1))
  const visiblePills = await pollForValue(async () => {
    const pixels = await sampleCanvasPixels(page, page.locator('#egui-canvas'), [
      { name: 'top', x: SAMPLE_X, y: wireY(1) - UI_CONSTANTS.GATE_SIZE / 2 - 9 },
      { name: 'bottom', x: SAMPLE_X, y: wireY(1) + UI_CONSTANTS.GATE_SIZE / 2 + 9 },
    ])
    return ['top', 'bottom'].filter((edge) =>
      pixelRgbDistance(pixels[edge], SPAN_RESIZE_HANDLE) < SPAN_RESIZE_HANDLE_TOLERANCE,
    ).length
  }, (count) => count === 2)
  expect(visiblePills).toBe(2)
})

test('Sample on a Bell state returns one possible joint outcome', async ({ page }) => {
  await openCircuit(page, [['H'], ['•', 'X'], ['Sample2']])
  expect([0, 3]).toContain(await sampleOutcome(page, 2))
})

test('Sample leaves a superposition unchanged without collapse', async ({ page }) => {
  await openCircuit(page, [['H'], ['•', 'X'], ['Sample2']])
  await sampleOutcome(page, 2)
  const state = Array.from(await readStateVector(page) as number[]).map((value) => Math.round(value * 1000) / 1000 || 0)
  expect(state).toEqual([0.707, 0, 0, 0, 0, 0, 0.707, 0])
})

test('Sample conditions on an on control in its column', async ({ page }) => {
  await openCircuit(page, [['H'], ['•', 'X'], ['•', 'Sample1']])
  expect(await sampleOutcome(page, 1)).toBe(1)
})

test('Sample conditions on odd parity in its column', async ({ page }) => {
  await openCircuit(page, [['H', '|0>', '|0>'], ['•', 1, 'X'], ['zpar', 'zpar', 'Sample1']])
  expect(await sampleOutcome(page, 1)).toBe(1)
})

test('Sample conditions on the opposite Z parity branch', async ({ page }) => {
  await openCircuit(page, [['H', '|1>', '|0>'], ['•', 1, 'X'], ['zpar', 'zpar', 'Sample1']])
  expect(await sampleOutcome(page, 1)).toBe(0)
})

test('Sample conditions on X-basis parity in its column', async ({ page }) => {
  await openCircuit(page, [['H', '|0>', '|0>'], ['•', 1, 'X'], ['H', 'H'], ['xpar', 'xpar', 'Sample1']])
  expect(await sampleOutcome(page, 1)).toBe(1)
})

test('Sample conditions on the opposite X parity branch', async ({ page }) => {
  await openCircuit(page, [['H', '|1>', '|0>'], ['•', 1, 'X'], ['H', 'H'], ['xpar', 'xpar', 'Sample1']])
  expect(await sampleOutcome(page, 1)).toBe(0)
})

test('Sample conditions on an anti-control in its column', async ({ page }) => {
  await openCircuit(page, [['H'], ['•', 'X'], ['◦', 'Sample1']])
  expect(await sampleOutcome(page, 1)).toBe(0)
})

test('Sample reports an unavailable outcome for impossible controls', async ({ page }) => {
  await openCircuit(page, [['|0>', '|1>'], ['•', 'Sample1']])
  expect(await sampleOutcome(page, 1)).toBe(0xffffffff)
})

test('Sample with impossible controls shows an unlit tile', async ({ page }) => {
  await openCircuit(page, [['|0>', '|1>'], ['•', 'Sample1']])
  await sampleOutcome(page, 1)
  const originY = PANEL_MARGIN + await getCircuitOriginY(page)
  const wireY = (wire: number) => originY + wire * UI_CONSTANTS.LINE_GAP
  const pixels = await sampleCanvasPixels(page, page.locator('#egui-canvas'), [
    { name: 'tile', x: SAMPLE_X, y: wireY(1) },
  ])
  expect(pixelRgbDistance(pixels.tile, UNLIT_FILL)).toBeLessThan(PIXEL_TOLERANCE)
})

test('Sample remains stable across repeated paints', async ({ page }) => {
  await openCircuit(page, [['H'], ['Sample1']])
  const first = await sampleOutcome(page, 1)
  await page.mouse.move(500, 500)
  await page.mouse.move(700, 600)
  expect(await sampleOutcome(page, 1)).toBe(first)
})

test('Sample readback rejects an out-of-range span', async ({ page }) => {
  await openCircuit(page, [['Sample1']])
  const [entry] = await readProbabilityDistributions(page)
  await expect(page.evaluate(async (gateId) => {
    const module = window.wasmBindings as unknown as {
      read_sample_display_outcome: (gateId: number, span: number) => Promise<number>
    }
    return module.read_sample_display_outcome(gateId, 9)
  }, entry.gateId)).rejects.toThrow('Sample span must be 1..8')
})

test('Sample readback rejects a span inconsistent with the actual display', async ({ page }) => {
  await openCircuit(page, [['Sample2']])
  const [entry] = await readProbabilityDistributions(page)
  await expect(page.evaluate(async (gateId) => {
    const module = window.wasmBindings as unknown as {
      read_sample_display_outcome: (gateId: number, span: number) => Promise<number>
    }
    return module.read_sample_display_outcome(gateId, 1)
  }, entry.gateId)).rejects.toThrow('Sample display span does not match live metadata')
})
