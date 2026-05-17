import { expect, test } from '@playwright/test'
import {
  dragPointer,
  getPaletteGateCenter,
  pixelRgbDistance,
  readChanceProbabilities,
  readMeasurementOutcomes,
  readStateVector,
  sampleCanvasPixels,
  UI_CONSTANTS,
  waitForStartupReady,
} from './support/egui-web-spec-helpers'

const GATE_SIZE = UI_CONSTANTS.GATE_SIZE
const SLOT_SPACING = UI_CONSTANTS.SLOT_SPACING
const LINE_LEFT_OFFSET = UI_CONSTANTS.LINE_LEFT_OFFSET
const LINE_Y = UI_CONSTANTS.LINE_Y
const LINE_GAP = UI_CONSTANTS.LINE_GAP
const EGUI_PANEL_MARGIN = 8
const CHANCE_PALETTE_INDEX = 20
const readCircuitColsFromHash = (url: string): unknown[] => JSON.parse(decodeURIComponent(new URL(url).hash.slice(1))).cols

const waitForHashCols = async (page: { url(): string; waitForTimeout(ms: number): Promise<void> }, expected: unknown[]): Promise<void> => {
  const expectedJson = JSON.stringify(expected)
  for (let attempt = 0; attempt < 50; attempt += 1) {
    if (JSON.stringify(readCircuitColsFromHash(page.url())) === expectedJson) return
    await page.waitForTimeout(50)
  }
  throw new Error(`URL hash columns did not become ${expectedJson}`)
}

const waitForChanceProbabilities = async (page: Parameters<typeof readChanceProbabilities>[0]): Promise<number[]> => {
  for (let attempt = 0; attempt < 50; attempt += 1) {
    const entries = await readChanceProbabilities(page)
    if (entries.length > 0) return entries[0].probabilities
    await page.waitForTimeout(50)
  }
  throw new Error('Chance probabilities did not become available')
}

const selectedColumnReadoutEvidence = async (page: Parameters<typeof readChanceProbabilities>[0]) => {
  const [chanceEntries, measurementEntries, state] = await Promise.all([
    readChanceProbabilities(page),
    readMeasurementOutcomes(page),
    readStateVector(page),
  ])
  const probs = chanceEntries[0]?.probabilities ?? []
  return {
    p0: Math.round((probs[0] ?? -1) * 1000) / 1000,
    p1: Math.round((probs[1] ?? -1) * 1000) / 1000,
    measurementCount: measurementEntries.length,
    selectedState: [
      Math.round(((state[0] as number | undefined) ?? -1) * 1000) / 1000,
      Math.round(((state[2] as number | undefined) ?? -1) * 1000) / 1000,
    ],
  }
}

const EXPECTED_SELECTED_COLUMN_READOUT = {
  p0: 0.5,
  p1: 0.5,
  measurementCount: 1,
  selectedState: [0.707, 0.707],
}

const waitForChancePercentLabelPixels = async (
  page: Parameters<typeof sampleCanvasPixels>[0],
  canvas: Parameters<typeof sampleCanvasPixels>[1],
  span: number,
): Promise<number> => {
  const gateLeft = EGUI_PANEL_MARGIN + LINE_LEFT_OFFSET + GATE_SIZE + SLOT_SPACING - GATE_SIZE / 2
  const gateTop = EGUI_PANEL_MARGIN + LINE_Y - GATE_SIZE / 2
  const gateHeight = (span - 1) * LINE_GAP + GATE_SIZE
  let last = 0
  for (let attempt = 0; attempt < 20; attempt += 1) {
    const screenshot = await canvas.screenshot({ type: 'png' })
    const box = await canvas.boundingBox()
    if (!box) throw new Error('expected egui canvas to be measurable')
    last = await page.evaluate(
      async ({ base64, cssWidth, cssHeight, gateLeft, gateTop, gateHeight }) => {
        const img = new Image()
        img.src = `data:image/png;base64,${base64}`
        await new Promise((resolve, reject) => {
          img.onload = () => resolve(null)
          img.onerror = () => reject(new Error('Failed to decode screenshot'))
        })
        const c = document.createElement('canvas')
        c.width = img.width
        c.height = img.height
        const ctx = c.getContext('2d', { willReadFrequently: true })
        if (!ctx) return 0
        ctx.drawImage(img, 0, 0)
        const scaleX = img.width / cssWidth
        const scaleY = img.height / cssHeight
        let textPixels = 0
        for (let y = Math.floor((gateTop + 4) * scaleY); y <= Math.floor((gateTop + gateHeight - 4) * scaleY); y += 1) {
          for (let x = Math.floor((gateLeft + 8) * scaleX); x <= Math.floor((gateLeft + 31) * scaleX); x += 1) {
            const [r, g, b] = ctx.getImageData(x, y, 1, 1).data
            if (r < 90 && g < 90 && b < 90) textPixels += 1
          }
        }
        return textPixels
      },
      { base64: screenshot.toString('base64'), cssWidth: box.width, cssHeight: box.height, gateLeft, gateTop, gateHeight },
    )
    if (last > 32) return last
    await page.waitForTimeout(50)
  }
  return last
}

const waitForChanceBarPixels = async (
  page: Parameters<typeof sampleCanvasPixels>[0],
  canvas: Parameters<typeof sampleCanvasPixels>[1],
): Promise<{ barIsBlue: boolean; emptyIsPaper: boolean }> => {
  const gateLeft = EGUI_PANEL_MARGIN + LINE_LEFT_OFFSET + GATE_SIZE + SLOT_SPACING - GATE_SIZE / 2
  const gateTop = EGUI_PANEL_MARGIN + LINE_Y - GATE_SIZE / 2
  let last = { barIsBlue: false, emptyIsPaper: false }
  for (let attempt = 0; attempt < 20; attempt += 1) {
    const screenshot = await canvas.screenshot({ type: 'png' })
    const box = await canvas.boundingBox()
    if (!box) throw new Error('expected egui canvas to be measurable')
    last = await page.evaluate(
      async ({ base64, cssWidth, cssHeight, gateLeft, gateTop }) => {
        const img = new Image()
        img.src = `data:image/png;base64,${base64}`
        await new Promise((resolve, reject) => {
          img.onload = () => resolve(null)
          img.onerror = () => reject(new Error('Failed to decode screenshot'))
        })
        const c = document.createElement('canvas')
        c.width = img.width
        c.height = img.height
        const ctx = c.getContext('2d', { willReadFrequently: true })
        if (!ctx) return { barIsBlue: false, emptyIsPaper: false }
        ctx.drawImage(img, 0, 0)
        const scaleX = img.width / cssWidth
        const scaleY = img.height / cssHeight
        const x0 = Math.floor((gateLeft + 2) * scaleX)
        const x1 = Math.floor((gateLeft + 38) * scaleX)
        const y0 = Math.floor((gateTop + 2) * scaleY)
        const y1 = Math.floor((gateTop + 38) * scaleY)
        let blue = 0
        let paper = 0
        for (let y = y0; y <= y1; y += 1) {
          for (let x = x0; x <= x1; x += 1) {
            const [r, g, b] = ctx.getImageData(x, y, 1, 1).data
            if (Math.abs(r - 102) + Math.abs(g - 160) + Math.abs(b - 200) < 16) blue += 1
            if (Math.abs(r - 255) + Math.abs(g - 252) + Math.abs(b - 240) < 16) paper += 1
          }
        }
        return { barIsBlue: blue > 80, emptyIsPaper: paper > 80 }
      },
      { base64: screenshot.toString('base64'), cssWidth: box.width, cssHeight: box.height, gateLeft, gateTop },
    )
    if (last.barIsBlue && last.emptyIsPaper) return last
    await page.waitForTimeout(50)
  }
  return last
}

const readoutVisualStabilityEvidence = async (
  page: Parameters<typeof sampleCanvasPixels>[0],
  canvas: Parameters<typeof sampleCanvasPixels>[1],
  box: { x: number; y: number },
): Promise<{ chanceDefaultFrames: number; missingMeasurementDigitFrames: number }> => {
  const chanceGateLeft = EGUI_PANEL_MARGIN + LINE_LEFT_OFFSET + GATE_SIZE + SLOT_SPACING - GATE_SIZE / 2
  const chanceGateTop = EGUI_PANEL_MARGIN + LINE_Y - GATE_SIZE / 2
  const chanceRowH = ((5 - 1) * LINE_GAP + GATE_SIZE) / 32
  const chanceProbe = { x: chanceGateLeft + 20, y: chanceGateTop + chanceRowH * 20.5 }
  const measureCenter = { x: EGUI_PANEL_MARGIN + LINE_LEFT_OFFSET + GATE_SIZE + SLOT_SPACING * 2, y: EGUI_PANEL_MARGIN + LINE_Y + 4 * LINE_GAP }
  let chanceDefaultFrames = 0
  let missingMeasurementDigitFrames = 0
  for (const column of [0, 1, 0, 2, 1, 0]) {
    await page.mouse.move(box.x + EGUI_PANEL_MARGIN + LINE_LEFT_OFFSET + GATE_SIZE + SLOT_SPACING * column, box.y + EGUI_PANEL_MARGIN + LINE_Y)
    await page.evaluate(() => new Promise<void>(resolve => requestAnimationFrame(() => resolve())))
    const screenshot = await canvas.screenshot({ type: 'png' })
    const canvasBox = await canvas.boundingBox()
    if (!canvasBox) throw new Error('expected egui canvas to be measurable')
    const evidence = await page.evaluate(
      async ({ base64, cssWidth, cssHeight, chanceProbe, measureCenter }) => {
        const img = new Image()
        img.src = `data:image/png;base64,${base64}`
        await new Promise((resolve, reject) => {
          img.onload = () => resolve(null)
          img.onerror = () => reject(new Error('Failed to decode screenshot'))
        })
        const c = document.createElement('canvas')
        c.width = img.width
        c.height = img.height
        const ctx = c.getContext('2d', { willReadFrequently: true })
        if (!ctx) return { chanceDefault: true, measurementDigitPixels: 0 }
        ctx.drawImage(img, 0, 0)
        const scaleX = img.width / cssWidth
        const scaleY = img.height / cssHeight
        const px = (xCss: number, yCss: number) => {
          const x = Math.round(xCss * scaleX)
          const y = Math.round(yCss * scaleY)
          return Array.from(ctx.getImageData(x, y, 1, 1).data.slice(0, 3)) as [number, number, number]
        }
        const dist = (rgb: [number, number, number], target: [number, number, number]) =>
          Math.abs(rgb[0] - target[0]) + Math.abs(rgb[1] - target[1]) + Math.abs(rgb[2] - target[2])
        const chanceDefault = dist(px(chanceProbe.x, chanceProbe.y), [102, 160, 200]) < 48
        let measurementDigitPixels = 0
        for (let y = measureCenter.y - 14; y <= measureCenter.y + 14; y += 1) {
          for (let x = measureCenter.x - 14; x <= measureCenter.x + 14; x += 1) {
            const rgb = px(x, y)
            if (dist(rgb, [32, 94, 166]) < 48 || dist(rgb, [175, 48, 41]) < 48) {
              measurementDigitPixels += 1
            }
          }
        }
        return { chanceDefault, measurementDigitPixels }
      },
      {
        base64: screenshot.toString('base64'),
        cssWidth: canvasBox.width,
        cssHeight: canvasBox.height,
        chanceProbe,
        measureCenter,
      },
    )
    if (evidence.chanceDefault) chanceDefaultFrames += 1
    if (evidence.measurementDigitPixels < 12) missingMeasurementDigitFrames += 1
  }
  return { chanceDefaultFrames, missingMeasurementDigitFrames }
}

const waitForChanceHoverEvidence = async (
  page: Parameters<typeof sampleCanvasPixels>[0],
  canvas: Parameters<typeof sampleCanvasPixels>[1],
): Promise<{ hoverBlue: boolean; popupText: boolean }> => {
  const gateLeft = EGUI_PANEL_MARGIN + LINE_LEFT_OFFSET + GATE_SIZE + SLOT_SPACING - GATE_SIZE / 2
  const selectedRowTop = EGUI_PANEL_MARGIN + LINE_Y
  let last = { hoverBlue: false, popupText: false }
  for (let attempt = 0; attempt < 20; attempt += 1) {
    const screenshot = await canvas.screenshot({ type: 'png' })
    const box = await canvas.boundingBox()
    if (!box) throw new Error('expected egui canvas to be measurable')
    const pixels = await page.evaluate(
      async ({ base64, cssWidth, cssHeight, gateLeft, selectedRowTop }) => {
        const img = new Image()
        img.src = `data:image/png;base64,${base64}`
        await new Promise((resolve, reject) => {
          img.onload = () => resolve(null)
          img.onerror = () => reject(new Error('Failed to decode screenshot'))
        })
        const c = document.createElement('canvas')
        c.width = img.width
        c.height = img.height
        const ctx = c.getContext('2d', { willReadFrequently: true })
        if (!ctx) return { hoverBlue: false, popupText: false }
        ctx.drawImage(img, 0, 0)
        const scaleX = img.width / cssWidth
        const scaleY = img.height / cssHeight
        const countMatching = (
          x0Css: number,
          x1Css: number,
          y0Css: number,
          y1Css: number,
          matches: (rgb: [number, number, number]) => boolean,
        ) => {
          let count = 0
          const x0 = Math.floor(x0Css * scaleX)
          const x1 = Math.floor(x1Css * scaleX)
          const y0 = Math.floor(y0Css * scaleY)
          const y1 = Math.floor(y1Css * scaleY)
          for (let y = y0; y <= y1; y += 1) {
            for (let x = x0; x <= x1; x += 1) {
              const [r, g, b] = ctx.getImageData(x, y, 1, 1).data
              if (matches([r, g, b])) count += 1
            }
          }
          return count
        }
        return {
          hoverBlue: countMatching(
            gateLeft + 2,
            gateLeft + 38,
            selectedRowTop + 2,
            selectedRowTop + 18,
            ([r, g, b]) => Math.abs(r - 32) + Math.abs(g - 94) + Math.abs(b - 166) < 24,
          ) > 40,
          popupText: countMatching(226, 390, 216, 292, ([r, g, b]) => r < 80 && g < 80 && b < 80) > 40,
        }
      },
      { base64: screenshot.toString('base64'), cssWidth: box.width, cssHeight: box.height, gateLeft, selectedRowTop },
    )
    last = pixels
    if (last.hoverBlue && last.popupText) return last
    await page.waitForTimeout(50)
  }
  return last
}

test('Chance display renders GPU probabilities and serializes the Quirk token', async ({ page }) => {
  await page.goto('/#' + encodeURIComponent(JSON.stringify({ cols: [['H'], ['Chance']] })))
  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  const probs = await waitForChanceProbabilities(page)
  const pixelResult = await waitForChanceBarPixels(page, canvas)

  expect({
    hashCols: readCircuitColsFromHash(page.url()),
    p0: Math.round(probs[0] * 1000) / 1000,
    p1: Math.round(probs[1] * 1000) / 1000,
    ...pixelResult,
  }).toEqual({
    hashCols: [['H'], ['Chance']],
    p0: 0.5,
    p1: 0.5,
    barIsBlue: true,
    emptyIsPaper: true,
  })
})

test('Chance4 displays GPU-rendered percentage labels', async ({ page }) => {
  await page.goto('/#' + encodeURIComponent(JSON.stringify({ cols: [['H', 'H', 'H', 'H'], ['Chance4']] })))
  await waitForStartupReady(page, { waitForStateVector: true })
  await waitForChanceProbabilities(page)

  const canvas = page.locator('#egui-canvas')
  const textPixels = await waitForChancePercentLabelPixels(page, canvas, 4)

  expect(textPixels > 32).toBe(true)
})

test('Chance5 keeps the Quirk-style bar-only display', async ({ page }) => {
  await page.goto('/#' + encodeURIComponent(JSON.stringify({ cols: [[1, 1, 1, 1, 'H'], ['Chance5']] })))
  await waitForStartupReady(page, { waitForStateVector: true })
  await waitForChanceProbabilities(page)

  const canvas = page.locator('#egui-canvas')
  const textPixels = await waitForChancePercentLabelPixels(page, canvas, 5)

  expect(textPixels < 8).toBe(true)
})

test('Chance hover highlights an outcome row and opens the popup', async ({ page }) => {
  await page.goto('/#' + encodeURIComponent(JSON.stringify({ cols: [['H'], ['Chance']] })))
  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  const box = await canvas.boundingBox()
  if (!box) throw new Error('expected egui canvas to be measurable')
  await page.mouse.move(box.x + EGUI_PANEL_MARGIN + LINE_LEFT_OFFSET + SLOT_SPACING + GATE_SIZE, box.y + EGUI_PANEL_MARGIN + LINE_Y + 8)
  const evidence = await waitForChanceHoverEvidence(page, canvas)

  expect(evidence).toEqual({ hoverBlue: true, popupText: true })
})

test('hovering columns does not flash readouts back to default bodies', async ({ page }) => {
  await page.goto('/#' + encodeURIComponent(JSON.stringify({ cols: [[1, 1, 1, 1, 'X'], ['Chance5'], [1, 1, 1, 1, 'Measure']] })))
  await waitForStartupReady(page, { waitForStateVector: true })
  await waitForChanceProbabilities(page)

  const canvas = page.locator('#egui-canvas')
  const box = await canvas.boundingBox()
  if (!box) throw new Error('expected egui canvas to be measurable')
  const evidence = await readoutVisualStabilityEvidence(page, canvas, box)

  expect(evidence).toEqual({ chanceDefaultFrames: 0, missingMeasurementDigitFrames: 0 })
})

test('selected earlier column keeps later readouts populated', async ({ page }) => {
  await page.goto('/#' + encodeURIComponent(JSON.stringify({ cols: [['H'], ['Chance'], ['Measure']] })))
  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  const box = await canvas.boundingBox()
  if (!box) throw new Error('expected egui canvas to be measurable')
  await page.mouse.move(box.x + LINE_LEFT_OFFSET + GATE_SIZE, box.y + LINE_Y)

  await expect
    .poll(() => selectedColumnReadoutEvidence(page))
    .toEqual(EXPECTED_SELECTED_COLUMN_READOUT)
})

test('Chance9 keeps tiny-row bars visible', async ({ page }) => {
  await page.goto('/#' + encodeURIComponent(JSON.stringify({ cols: [[1, 'H'], ['Chance9']] })))
  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  const probs = await waitForChanceProbabilities(page)
  const gateTop = LINE_Y - GATE_SIZE / 2 + 8
  const pixels = await sampleCanvasPixels(page, canvas, [{
    name: 'outcome128Bar',
    x: LINE_LEFT_OFFSET + SLOT_SPACING + GATE_SIZE - 2,
    y: gateTop + ((9 - 1) * LINE_GAP + GATE_SIZE) / 4,
  }])

  expect({
    p128: Math.round(probs[128] * 1000) / 1000,
    barVisible: pixelRgbDistance(pixels.outcome128Bar, [102, 160, 200, 255]) < 64,
  }).toEqual({ p128: 0.5, barVisible: true })
})

test('Chance palette drop can resize to Chance3', async ({ page }) => {
  await page.goto('/')
  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  const box = await canvas.boundingBox()
  if (!box) throw new Error('expected egui canvas to be measurable')
  const cssWidth = box.width
  const source = getPaletteGateCenter(cssWidth, CHANCE_PALETTE_INDEX)
  const target = { x: LINE_LEFT_OFFSET + GATE_SIZE, y: LINE_Y }
  await dragPointer(page, source, target)
  await waitForHashCols(page, [['Chance']])

  const handle = { x: target.x, y: LINE_Y + GATE_SIZE / 2 + 10 }
  await page.mouse.move(box.x + handle.x, box.y + handle.y)
  await page.mouse.down()
  await page.mouse.move(box.x + handle.x, box.y + handle.y + 2 * LINE_GAP, { steps: 8 })
  await page.mouse.up()
  await waitForHashCols(page, [['Chance3']])

  expect(readCircuitColsFromHash(page.url())).toEqual([['Chance3']])
})
