import { expect, test, type Page } from '@playwright/test'
import { dragPointer, readStateVector, waitForStartupReady, waitForStateVectorApprox } from './support/web-spec-helpers'

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
    await waitForStateVectorApprox(page, [Math.SQRT1_2, 0, Math.SQRT1_2, 0])
    await page.keyboard.down('Shift')
    await page.mouse.move(side === 'left' ? 156 : 180, 264)
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
  await page.keyboard.down('Shift')
  await page.mouse.move(180, 264)
  await page.mouse.down()
  await waitForStateVectorApprox(page, [1, 0, 0, 0])
  await expect(page).toHaveScreenshot('tentative-h-copy-state.png', {
    clip: { x: 352, y: 532, width: 576, height: 204 },
  })
  await page.keyboard.press('Escape')
  await page.mouse.up()
  await page.keyboard.up('Shift')
})

test('shift drag updates state vector while the copy is held over a slot', async ({ page }) => {
  await page.goto(`/#${encodeURIComponent(JSON.stringify({ cols: [['H']] }))}`)
  await waitForStartupReady(page, { waitForStateVector: true })
  await page.keyboard.down('Shift')
  await dragPointer(page, { x: 180, y: 264 }, { x: 330, y: 264 }, 8, false)
  await expect.poll(() => roundedState(page)).toEqual([1, 0, 0, 0])
  await page.keyboard.press('Escape')
  await page.mouse.up()
  await page.keyboard.up('Shift')
})

test('shift copy includes the tentative gate even with a fixed column selected', async ({ page }) => {
  await page.goto(`/#${encodeURIComponent(JSON.stringify({ cols: [['H']] }))}`)
  await waitForStartupReady(page, { waitForStateVector: true })
  await page.mouse.click(170, 320)
  await page.keyboard.down('Shift')
  await page.mouse.move(180, 264)
  await page.mouse.down()
  await expect.poll(() => roundedState(page)).toEqual([1, 0, 0, 0])
  await page.keyboard.press('Escape')
  await page.mouse.up()
  await page.keyboard.up('Shift')
})
