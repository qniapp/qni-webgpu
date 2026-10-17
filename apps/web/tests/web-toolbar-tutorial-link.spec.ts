import { expect, test, type Page } from '@playwright/test'
import { waitForStartupReady } from './support/web-spec-helpers'

const TUTORIAL_URL = 'https://qniapp.github.io/qni-tutorial-webgpu/'
const TOOLTIP = 'Open tutorial in a new tab'

type TutorialGeometry = {
  left: number
  right: number
  top: number
  bottom: number
}

const tutorialGeometry = async (page: Page): Promise<TutorialGeometry> => {
  await page.waitForFunction(() => typeof (window as any).__qniToolbarTutorialGeometryJson === 'string')
  return page.evaluate(() => JSON.parse((window as any).__qniToolbarTutorialGeometryJson))
}

for (const mode of ['local', 'gpu']) {
  test(`Tutorial opens the exact tutorial URL in a new tab in ${mode} mode`, async ({ page, context }) => {
    await context.route('https://qniapp.github.io/**', route => route.fulfill({
      contentType: 'text/html',
      body: '<!doctype html><title>Tutorial</title>',
    }))
    await page.goto(mode === 'gpu' ? '/?mode=gpu' : '/')
    await waitForStartupReady(page)
    const rect = await tutorialGeometry(page)
    const popupPromise = page.waitForEvent('popup')
    await page.locator('#egui-canvas').click({
      position: { x: (rect.left + rect.right) / 2, y: (rect.top + rect.bottom) / 2 },
    })
    const popup = await popupPromise
    await expect(popup).toHaveURL(TUTORIAL_URL)
  })
}

const hoverTutorial = async (page: Page) => {
  await page.goto('/')
  await waitForStartupReady(page)
  const rect = await tutorialGeometry(page)
  const canvas = page.locator('#egui-canvas')
  await canvas.hover({
    position: { x: (rect.left + rect.right) / 2, y: (rect.top + rect.bottom) / 2 },
  })
  return canvas
}

test('Tutorial shows its tooltip on hover', async ({ page }) => {
  await hoverTutorial(page)
  await expect.poll(() => page.evaluate(() => (window as any).__qniToolbarTooltipText)).toBe(TOOLTIP)
})

test('Tutorial uses a pointing-hand cursor on hover', async ({ page }) => {
  const canvas = await hoverTutorial(page)
  await expect(canvas).toHaveCSS('cursor', 'pointer')
})

test('Tutorial is absent from the embed toolbar', async ({ page }) => {
  await page.route('**/tutorial-link-embed', route => route.fulfill({
    contentType: 'text/html',
    body: `<!doctype html><html><body style="margin:0">
      <canvas id="egui-canvas" style="width:1000px;height:800px"></canvas>
      <script type="module">
        import init, { start_embed } from '/qni-web.js';
        await init();
        window.runner = await start_embed(document.querySelector('canvas'), '{"cols":[]}', true, undefined, undefined, false);
      </script></body></html>`,
  }))
  await page.goto('/tutorial-link-embed')
  await page.waitForFunction(() => (window as any).runner)
  await page.evaluate(() => new Promise<void>(resolve => {
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
  }))
  expect(await page.evaluate(() => (window as any).__qniToolbarTutorialGeometryJson)).toBeUndefined()
})
