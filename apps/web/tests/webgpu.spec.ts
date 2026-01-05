import { test, expect } from '@playwright/test'

const gateCases = [
  { gate: 'X', expected: [0, 0, 1, 0] },
  { gate: 'H', expected: [1 / Math.sqrt(2), 0, 1 / Math.sqrt(2), 0] },
  { gate: 'Y', expected: [0, 0, 0, 1] },
  { gate: 'Z', expected: [1, 0, 0, 0] },
  { gate: 'S', expected: [1, 0, 0, 0] },
  { gate: 'T', expected: [1, 0, 0, 0] },
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
    expect(canvasSize).toEqual({ width: '800', height: '600' })

    const statusText = await page.$eval('#status', (el) => el.textContent?.trim() ?? '')
    expect(statusText).toBe('')

    await page.waitForFunction(
      () => (window as { __renderDone?: boolean }).__renderDone === true,
      null,
      { timeout: 8000 }
    )
    const statusAfter = await page.$eval('#status', (el) => el.textContent?.trim() ?? '')
    expect(statusAfter).toBe('')
    const vertexCount = await page.evaluate(() => (window as { __vertexCount?: number }).__vertexCount ?? 0)
    expect(vertexCount).toBeGreaterThan(0)
    const stateVector = await page.evaluate(() => (window as { __stateVector?: number[] }).__stateVector ?? [])
    expect(stateVector.length).toBe(4)
    expect(stateVector[0]).toBeCloseTo(expected[0], 5)
    expect(stateVector[1]).toBeCloseTo(expected[1], 5)
    expect(stateVector[2]).toBeCloseTo(expected[2], 5)
    expect(stateVector[3]).toBeCloseTo(expected[3], 5)
    await page.screenshot({ path: `/tmp/qni-webgpu-webgpu-${gate}.png` })
  })
}
