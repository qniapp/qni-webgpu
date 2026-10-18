import { expect, test, type Page } from '@playwright/test'
import {
  dragPointer,
  getCircuitOriginY,
  isDragPreviewFill,
  readStateVector,
  sampleCanvasPixels,
  waitForStartupReady,
  waitForStateVectorApprox,
  waitForValue,
} from './support/web-spec-helpers'

test.use({ viewport: { width: 1280, height: 800 } })

async function roundedState(page: Page): Promise<number[]> {
  return (await readStateVector(page)).map(value => {
    if (typeof value !== 'number') throw new Error('expected a numeric state vector component')
    return Math.round(value * 1e6) / 1e6
  })
}

for (const side of ['left', 'right'] as const) {
  test(`shift ${side} copy updates state vector before release`, async ({ page }) => {
    await page.goto(`/#${encodeURIComponent(JSON.stringify({ cols: [['H']] }))}`)
    await waitForStartupReady(page, { waitForStateVector: true })
    const circuitY = await getCircuitOriginY(page)
    await waitForStateVectorApprox(page, [Math.SQRT1_2, 0, Math.SQRT1_2, 0])
    await page.keyboard.down('Shift')
    await page.mouse.move(side === 'left' ? 156 : 180, circuitY + 8)
    await page.mouse.down()
    await expect.poll(() => roundedState(page)).toEqual([1, 0, 0, 0])
    await page.keyboard.press('Escape')
    await page.mouse.up()
    await page.keyboard.up('Shift')
  })
}

test('state panel shows the tentative H copy result', async ({ page }) => {
  await page.goto(`/#${encodeURIComponent(JSON.stringify({ cols: [['H']] }))}`)
  await waitForStartupReady(page, { waitForStateVector: true })
  const circuitY = await getCircuitOriginY(page)
  await page.keyboard.down('Shift')
  await page.mouse.move(180, circuitY + 8)
  await page.mouse.down()
  await waitForStateVectorApprox(page, [1, 0, 0, 0])
  const viewport = page.viewportSize()
  if (!viewport) throw new Error('expected a fixed viewport')
  await expect(page).toHaveScreenshot('tentative-h-copy-state.png', {
    clip: { x: 352, y: viewport.height - 268, width: 576, height: 204 },
  })
  await page.keyboard.press('Escape')
  await page.mouse.up()
  await page.keyboard.up('Shift')
})

test('shift drag updates state vector while the copy is held over a slot', async ({ page }) => {
  await page.goto(`/#${encodeURIComponent(JSON.stringify({ cols: [['H']] }))}`)
  await waitForStartupReady(page, { waitForStateVector: true })
  const circuitY = await getCircuitOriginY(page)
  await page.keyboard.down('Shift')
  await dragPointer(page, { x: 180, y: circuitY + 8 }, { x: 330, y: circuitY + 8 }, 8, false)
  await expect.poll(() => roundedState(page)).toEqual([1, 0, 0, 0])
  await page.keyboard.press('Escape')
  await page.mouse.up()
  await page.keyboard.up('Shift')
})

async function waitForCopyPreview(page: Page, side: 'left' | 'right'): Promise<void> {
  const circuitY = await getCircuitOriginY(page)
  await waitForValue(async () => {
    const pixels = await sampleCanvasPixels(page, page.locator('#egui-canvas'), [
      { name: 'copy', x: side === 'left' ? 142 : 198, y: circuitY + 22 },
    ])
    return isDragPreviewFill(pixels.copy)
  }, value => value, 'expected the held copy preview before cancellation')
}

for (const side of ['left', 'right'] as const) {
  test(`Escape cancels the ${side} copy and restores the fixed column state`, async ({ page }) => {
    await page.goto(`/#${encodeURIComponent(JSON.stringify({ cols: [['H'], ['H']] }))}`)
    await waitForStartupReady(page, { waitForStateVector: true })
    const circuitY = await getCircuitOriginY(page)
    await page.mouse.click(170, circuitY + 64)
    await page.mouse.move(1000, 400)
    await waitForStateVectorApprox(page, [Math.SQRT1_2, 0, Math.SQRT1_2, 0])
    await page.keyboard.down('Shift')
    await page.mouse.move(side === 'left' ? 156 : 180, circuitY + 8)
    await page.mouse.down()
    await waitForCopyPreview(page, side)
    await page.keyboard.press('Escape')
    await page.mouse.up()
    await page.keyboard.up('Shift')
    await page.mouse.move(1000, 400)
    await expect.poll(() => roundedState(page)).toEqual([0.707107, 0, 0.707107, 0])
  })
}

test('Escape restores the fixed column selection and state panel visually', async ({ page }) => {
  await page.goto(`/#${encodeURIComponent(JSON.stringify({ cols: [['H'], ['H']] }))}`)
  await waitForStartupReady(page, { waitForStateVector: true })
  const circuitY = await getCircuitOriginY(page)
  await page.mouse.click(170, circuitY + 64)
  await page.mouse.move(1000, 400)
  await waitForStateVectorApprox(page, [Math.SQRT1_2, 0, Math.SQRT1_2, 0])
  await page.keyboard.down('Shift')
  await page.mouse.move(180, circuitY + 8)
  await page.mouse.down()
  await waitForCopyPreview(page, 'right')
  await page.keyboard.press('Escape')
  await page.mouse.up()
  await page.keyboard.up('Shift')
  await page.mouse.move(1000, 400)
  await waitForStateVectorApprox(page, [Math.SQRT1_2, 0, Math.SQRT1_2, 0])
  await expect(page).toHaveScreenshot('cancelled-copy-fixed-column.png')
})

test('shift copy includes the tentative gate even with a fixed column selected', async ({ page }) => {
  await page.goto(`/#${encodeURIComponent(JSON.stringify({ cols: [['H']] }))}`)
  await waitForStartupReady(page, { waitForStateVector: true })
  const circuitY = await getCircuitOriginY(page)
  await page.mouse.click(170, circuitY + 64)
  await page.keyboard.down('Shift')
  await page.mouse.move(180, circuitY + 8)
  await page.mouse.down()
  await expect.poll(() => roundedState(page)).toEqual([1, 0, 0, 0])
  await page.keyboard.press('Escape')
  await page.mouse.up()
  await page.keyboard.up('Shift')
})
