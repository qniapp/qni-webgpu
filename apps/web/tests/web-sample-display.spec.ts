import { expect, test, type Page } from '@playwright/test'
import {
  pixelRgbDistance,
  readProbabilityDistributions,
  readStateVector,
  sampleCanvasPixels,
  UI_CONSTANTS,
  waitForStartupReady,
  waitForValue,
} from './support/web-spec-helpers'

type Column = (string | number)[]
const PANEL_MARGIN = 8
const SAMPLE_FILL: [number, number, number, number] = [146, 191, 219, 255]
const SURFACE: [number, number, number, number] = [255, 252, 240, 255]
const PIXEL_TOLERANCE = 24

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

  test(`Sample${span} paints one highlighted digit per wire`, async ({ page }) => {
    const bits = Array.from({ length: span }, (_, wire) => wire % 2 === 0 ? 1 : 0)
    await openCircuit(page, [bits.map((bit) => `|${bit}>`), [`Sample${span}`]])
    await sampleOutcome(page, span)
    const centerX = PANEL_MARGIN + UI_CONSTANTS.LINE_LEFT_OFFSET + UI_CONSTANTS.GATE_SIZE + UI_CONSTANTS.SLOT_SPACING
    const pixels = await sampleCanvasPixels(page, page.locator('#egui-canvas'), bits.flatMap((_, wire) => [
      { name: `left${wire}`, x: centerX - UI_CONSTANTS.GATE_SIZE / 2 + 4, y: PANEL_MARGIN + UI_CONSTANTS.LINE_Y + wire * UI_CONSTANTS.LINE_GAP },
      { name: `right${wire}`, x: centerX + UI_CONSTANTS.GATE_SIZE / 2 - 4, y: PANEL_MARGIN + UI_CONSTANTS.LINE_Y + wire * UI_CONSTANTS.LINE_GAP },
    ]))
    expect(bits.every((bit, wire) => ['left', 'right'].every((side) =>
      pixelRgbDistance(pixels[`${side}${wire}`], bit ? SAMPLE_FILL : SURFACE) < PIXEL_TOLERANCE,
    ))).toBe(true)
  })
}

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
    return module.read_sample_display_outcome(gateId, 4)
  }, entry.gateId)).rejects.toThrow('Sample span must be 1..3')
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
