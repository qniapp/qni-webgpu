import { expect, test } from '@playwright/test'
import { dragPointer, isDragPreviewFill, sampleCanvasPixels, waitForStartupReady } from './support/web-spec-helpers'

test.use({ viewport: { width: 1280, height: 800 } })

test('shift copy insertion preview stays above the moved state panel', async ({ page }) => {
  await page.goto(`/#${encodeURIComponent(JSON.stringify({ cols: [['H'], ['X']] }))}`)
  await waitForStartupReady(page, { waitForStateVector: true })
  await dragPointer(page, { x: 640, y: 552 }, { x: 470, y: 252 })
  await page.keyboard.down('Shift')
  await page.mouse.move(180, 264)
  await page.mouse.down()
  await page.waitForTimeout(100)
  const pixels = await sampleCanvasPixels(page, page.locator('#egui-canvas'), [
    { name: 'copy', x: 198, y: 278 },
  ])
  expect(isDragPreviewFill(pixels.copy)).toBe(true)
  await page.mouse.up()
  await page.keyboard.up('Shift')
})

for (const side of ['left', 'right', 'center'] as const) {
  test(`shift click ${side} insertion preview visual snapshot`, async ({ page }) => {
    await page.goto(`/#${encodeURIComponent(JSON.stringify({ cols: [['H'], ['X']] }))}`)
    await waitForStartupReady(page, { waitForStateVector: true })
    await page.keyboard.down('Shift')
    await page.mouse.move(side === 'left' ? 156 : side === 'right' ? 180 : 170, 264)
    await page.mouse.down()
    await page.waitForTimeout(100)
    await expect(page).toHaveScreenshot(`shift-copy-${side}.png`, {
      clip: { x: 120, y: 232, width: 152, height: 64 },
    })
    await page.mouse.up()
    await page.keyboard.up('Shift')
  })
}

for (const display of ['Probability', 'Amps4', 'Density2']) {
  test(`shift copy preserves neighboring ${display} GPU rendering`, async ({ page }) => {
    await page.goto(`/#${encodeURIComponent(JSON.stringify({ cols: [['H'], [display]] }))}`)
    await waitForStartupReady(page, { waitForStateVector: true })
    await page.keyboard.down('Shift')
    await page.mouse.move(180, 264)
    await page.mouse.down()
    await page.waitForTimeout(100)
    await expect(page).toHaveScreenshot(`shift-copy-${display}.png`, {
      clip: { x: 120, y: 232, width: 400, height: 256 },
    })
    await page.mouse.up()
    await page.keyboard.up('Shift')
  })
}

test('shift copy panel overlap visual snapshot', async ({ page }) => {
  await page.goto(`/#${encodeURIComponent(JSON.stringify({ cols: [['H'], ['X']] }))}`)
  await waitForStartupReady(page, { waitForStateVector: true })
  await dragPointer(page, { x: 640, y: 552 }, { x: 470, y: 252 })
  await page.keyboard.down('Shift')
  await page.mouse.move(180, 264)
  await page.mouse.down()
  await page.waitForTimeout(100)
  await expect(page).toHaveScreenshot('shift-copy-panel-overlap.png', {
    clip: { x: 120, y: 232, width: 152, height: 64 },
  })
  await page.mouse.up()
  await page.keyboard.up('Shift')
})
