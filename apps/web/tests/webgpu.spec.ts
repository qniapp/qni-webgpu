import { test, expect } from '@playwright/test'

const gateCases = [
  { gate: 'H', expected: [1 / Math.sqrt(2), 0, 0, 0, 1 / Math.sqrt(2), 0, 0, 0] },
  { gate: 'X', expected: [0, 0, 0, 0, 1, 0, 0, 0] },
  { gate: 'Y', expected: [0, 0, 0, 0, 0, 1, 0, 0] },
  { gate: 'Z', expected: [1, 0, 0, 0, 0, 0, 0, 0] },
  { gate: '√X', expected: [0.5, 0.5, 0, 0, 0.5, -0.5, 0, 0] },
  { gate: 'S', expected: [1, 0, 0, 0, 0, 0, 0, 0] },
  { gate: 'S†', expected: [1, 0, 0, 0, 0, 0, 0, 0] },
  { gate: 'T', expected: [1, 0, 0, 0, 0, 0, 0, 0] },
  { gate: 'T†', expected: [1, 0, 0, 0, 0, 0, 0, 0] },
]

for (const { gate, expected } of gateCases) {
  test(`webgpu gate ${gate} renders without status errors`, async ({ page }) => {
    await page.addInitScript(() => {
      window.__captureFrame = true
      window.__captureStateVector = true
    })
    await page.goto(`/?gate=${gate}`)

    const gpuAvailable = await page.evaluate(() => Boolean(navigator.gpu))
    expect(gpuAvailable).toBe(true)

    await page.waitForSelector('#gfx', { timeout: 3000 })
    const canvasSize = await page.$eval('#gfx', (el) => ({
      width: el.getAttribute('width'),
      height: el.getAttribute('height'),
    }))
    const viewport = page.viewportSize()
    expect(canvasSize).toEqual({ width: String(viewport?.width), height: String(viewport?.height) })

    const statusText = await page.$eval('#status', (el) => el.textContent?.trim() ?? '')
    expect(statusText).toBe('')

    await page.waitForFunction(
      () => (window as { __renderDone?: boolean }).__renderDone === true,
      null,
      { timeout: 8000 }
    )

    const paletteIndex = ['H', 'X', 'Y', 'Z', '√X', 'S', 'S†', 'T', 'T†'].indexOf(gate)
    const PALETTE_SIZE = 32
    const PALETTE_GAP = 16
    const PALETTE_ROW_Y = 12
    const CANVAS_WIDTH = viewport?.width ?? 800
    const LINE_LEFT = 80
    const GATE_SIZE = 32
    const SLOT_LEFT = LINE_LEFT + GATE_SIZE
    const LINE_Y = 160
    const paletteWidth = 9 * PALETTE_SIZE + 8 * PALETTE_GAP
    const paletteStartX = (CANVAS_WIDTH - paletteWidth) / 2
    const sourceX = paletteStartX + paletteIndex * (PALETTE_SIZE + PALETTE_GAP) + PALETTE_SIZE / 2
    const sourceY = PALETTE_ROW_Y + PALETTE_SIZE / 2
    const targetX = SLOT_LEFT
    const targetY = LINE_Y

    const canvasBox = await page.locator('#gfx').boundingBox()
    expect(canvasBox).not.toBeNull()
    const offsetX = canvasBox?.x ?? 0
    const offsetY = canvasBox?.y ?? 0

    await page.mouse.move(sourceX + offsetX, sourceY + offsetY)
    await page.mouse.down()
    await page.mouse.move(targetX + offsetX, targetY + offsetY)
    await page.mouse.up()

    await page.waitForTimeout(200)
    const statusAfter = await page.$eval('#status', (el) => el.textContent?.trim() ?? '')
    expect(statusAfter).toBe('')
    const vertexCount = await page.evaluate(() => (window as { __vertexCount?: number }).__vertexCount ?? 0)
    expect(vertexCount).toBeGreaterThan(0)
    const stateVector = await page.evaluate(() => (window as { __stateVector?: number[] }).__stateVector ?? [])
    expect(stateVector.length).toBe(8)
    expected.forEach((value, index) => {
      expect(stateVector[index]).toBeCloseTo(value, 5)
    })
    await page.screenshot({ path: `/tmp/qni-webgpu-webgpu-${gate}.png` })
  })
}
