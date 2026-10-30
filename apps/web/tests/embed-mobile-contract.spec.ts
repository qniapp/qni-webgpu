import { test, expect, type Page } from '@playwright/test'
import { EmbedContractHost } from '../test-support/embed-api-contract'
import { collectBrowserErrors } from '../test-support/browser-errors'
import { writeFile } from 'node:fs/promises'

const circuit = JSON.stringify({ cols: Array.from({ length: 14 }, (_, i) =>
  i === 0 ? ['H', 1, 1, 'X'] : ['X']) })

class MobileEmbed {
  static async start(page: Page, initialCircuit = circuit, palette = ['H', 'X'], showStatePanel = false) {
    await page.context().grantPermissions(['local-network-access'])
    await page.route('**/mobile-article', route => route.fulfill({
      contentType: 'text/html',
      body: `<html><head><meta name="viewport" content="width=device-width, initial-scale=1"></head>
        <body style="margin:0"><div style="height:40px">Tutorial</div>
        <canvas style="display:block;width:390px;height:440px"></canvas>
        <div style="height:1400px">Article continues</div><script type="module">
        try {
          const {startEmbed} = await import('${EmbedContractHost.assetURL}qni-embed.mjs');
          window.runner = await startEmbed(document.querySelector('canvas'), ${JSON.stringify(initialCircuit)},
            {showStatePanel:${showStatePanel}, palette:${JSON.stringify(palette)}, maxWireCount:4});
          window.started = true;
        } catch(error) { window.startError = String(error); }
        </script></body></html>`,
    }))
    await page.goto('/mobile-article')
    await page.waitForFunction(() => (window as any).started || (window as any).startError)
    const error = await page.evaluate(() => (window as any).startError)
    if (error) throw Error(error)
    await page.waitForFunction(async () => {
      try { return (await (window as any).runner.readStateVector()).length === 32 } catch { return false }
    })
  }

  static async swipe(page: Page, from: { x: number, y: number }, to: { x: number, y: number }, intervalMs = 25, end: 'touchEnd' | 'touchCancel' = 'touchEnd') {
    const cdp = await page.context().newCDPSession(page)
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ ...from, id: 1 }] })
    for (let i = 1; i <= 12; i++) {
      await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{
        x: from.x + (to.x - from.x) * i / 12,
        y: from.y + (to.y - from.y) * i / 12, id: 1,
      }] })
      if (intervalMs) await page.waitForTimeout(intervalMs)
    }
    await cdp.send('Input.dispatchTouchEvent', { type: end, touchPoints: [] })
    await cdp.detach()
    await page.waitForTimeout(200)
  }

  static async allowPageScrollInBothDirections(page: Page) {
    await page.evaluate(() => {
      const spacer = document.createElement('div')
      spacer.style.height = '400px'
      document.body.prepend(spacer)
      scrollTo(0, 400)
    })
  }

  static async selectedStep(page: Page) {
    return page.evaluate(() => (window as any).runner.selectedStep())
  }
}

test.use({ viewport: { width: 390, height: 844 }, deviceScaleFactor: 2, hasTouch: true, isMobile: true })
test.beforeAll(() => EmbedContractHost.listen())
test.afterAll(() => EmbedContractHost.close())
test.beforeEach(async ({ page }) => MobileEmbed.start(page))

test('mobile embed starts at step zero', async ({ page }) => {
  expect(await MobileEmbed.selectedStep(page)).toBe(0)
})

for (const [name, point, step] of [
  ['step', { x: 144, y: 335 }, 1],
  ['blank space', { x: 330, y: 420 }, 4],
] as const) {
  test(`mobile stationary ${name} tap does not synthesize host mouse events`, async ({ page }, testInfo) => {
    const before = await page.evaluate(() => (window as any).runner.circuitJSON())
    await page.evaluate(() => {
      const events: string[] = []
      const w = window as any
      w.hostMouseEvents = events
      for (const type of ['mousemove', 'mousedown', 'mouseup', 'click']) {
        document.addEventListener(type, () => events.push(type), { capture: true })
      }
    })
    await page.touchscreen.tap(point.x, point.y)
    await page.waitForTimeout(200)
    const after = await page.evaluate(() => ({
      mouseEvents: (window as any).hostMouseEvents,
      step: (window as any).runner.selectedStep(),
      circuit: (window as any).runner.circuitJSON(),
    }))
    await writeFile(testInfo.outputPath('post-tap.json'), JSON.stringify(after, null, 2))
    expect(after).toEqual({ mouseEvents: [], step, circuit: before })
  })
}

test('mobile empty-area horizontal swipe reveals later steps', async ({ page }) => {
  await MobileEmbed.swipe(page, { x: 330, y: 335 }, { x: 90, y: 335 })
  if (process.env.QNI_MOBILE_SCREENSHOT) await page.screenshot({ path: process.env.QNI_MOBILE_SCREENSHOT })
  await page.touchscreen.tap(144, 335)
  await expect.poll(() => MobileEmbed.selectedStep(page)).toBeGreaterThan(1)
})

test('mobile gate drag remains editable', async ({ page }) => {
  await MobileEmbed.swipe(page, { x: 88, y: 256 }, { x: 200, y: 312 })
  await expect.poll(() => page.evaluate(() => JSON.parse((window as any).runner.circuitJSON()).cols))
    .toEqual([[1, 1, 1, 'X'], ['X'], ['X', 'H'], ...Array.from({ length: 11 }, () => ['X'])])
})

test('mobile vertical swipe scrolls the article', async ({ page }) => {
  await MobileEmbed.swipe(page, { x: 330, y: 420 }, { x: 330, y: 200 })
  await expect.poll(() => page.evaluate(() => scrollY)).toBeGreaterThan(50)
})

test('mobile vertical article scroll preserves authored circuit JSON', async ({ page }) => {
  const before = await page.evaluate(() => (window as any).runner.circuitJSON())
  await MobileEmbed.swipe(page, { x: 330, y: 420 }, { x: 330, y: 200 })
  expect(await page.evaluate(() => (window as any).runner.circuitJSON())).toBe(before)
})

for (const [name, from, to, interval, end] of [
  ['article scroll', { x: 330, y: 420 }, { x: 330, y: 200 }, 25, 'touchEnd'],
  ['rapid article scroll', { x: 330, y: 420 }, { x: 330, y: 200 }, 0, 'touchEnd'],
  ['horizontal pan', { x: 330, y: 335 }, { x: 90, y: 335 }, 25, 'touchEnd'],
  ['gate drag', { x: 88, y: 256 }, { x: 200, y: 312 }, 25, 'touchEnd'],
  ['palette drag', { x: 171, y: 148 }, { x: 200, y: 368 }, 25, 'touchEnd'],
  ['explicit cancellation', { x: 330, y: 335 }, { x: 320, y: 335 }, 0, 'touchCancel'],
] as const) {
  test(`mobile ${name} has no console errors or browser interventions`, async ({ page }, testInfo) => {
    const { messages, cdp } = await collectBrowserErrors(page)
    await MobileEmbed.swipe(page, from, to, interval, end)
    await writeFile(testInfo.outputPath('touch-browser-log.json'), JSON.stringify(messages, null, 2))
    await testInfo.attach('touch-browser-log', { path: testInfo.outputPath('touch-browser-log.json'), contentType: 'application/json' })
    await cdp.detach()
    expect(messages).toEqual([])
  })
}

test('mobile tap still selects after native article scrolling cancels a touch', async ({ page }) => {
  await MobileEmbed.swipe(page, { x: 330, y: 420 }, { x: 330, y: 200 })
  await page.evaluate(() => scrollTo(0, 0))
  await page.touchscreen.tap(144, 335)
  await expect.poll(() => MobileEmbed.selectedStep(page)).toBe(1)
})

test('mobile tap still selects after an explicit touch cancellation', async ({ page }) => {
  await MobileEmbed.swipe(page, { x: 330, y: 420 }, { x: 330, y: 420 }, 0, 'touchCancel')
  await page.touchscreen.tap(144, 335)
  await expect.poll(() => MobileEmbed.selectedStep(page)).toBe(1)
})

test('mobile cancelled step press does not select the step', async ({ page }) => {
  await MobileEmbed.swipe(page, { x: 144, y: 335 }, { x: 144, y: 335 }, 0, 'touchCancel')
  expect(await MobileEmbed.selectedStep(page)).toBe(0)
})

for (const [name, from, to] of [
  ['gate', { x: 88, y: 256 }, { x: 200, y: 312 }],
  ['palette', { x: 171, y: 148 }, { x: 200, y: 368 }],
] as const) {
  test(`mobile cancelled ${name} drag restores the authored circuit`, async ({ page }) => {
    const before = await page.evaluate(() => (window as any).runner.circuitJSON())
    await MobileEmbed.swipe(page, from, to, 25, 'touchCancel')
    expect(await page.evaluate(() => (window as any).runner.circuitJSON())).toBe(before)
  })
}

test('mobile released drag does not follow the next unrelated movement', async ({ page }) => {
  await MobileEmbed.swipe(page, { x: 88, y: 256 }, { x: 200, y: 312 })
  const released = await page.evaluate(() => (window as any).runner.circuitJSON())
  await MobileEmbed.swipe(page, { x: 330, y: 420 }, { x: 330, y: 400 }, 0, 'touchCancel')
  expect(await page.evaluate(() => (window as any).runner.circuitJSON())).toBe(released)
})

test('mobile rapid vertical touch moves still scroll the article', async ({ page }) => {
  await MobileEmbed.swipe(page, { x: 330, y: 420 }, { x: 330, y: 200 }, 0)
  await expect.poll(() => page.evaluate(() => scrollY)).toBeGreaterThan(50)
})

test('mobile vertical scroll does not require an app frame after touchstart', async ({ page }) => {
  await page.evaluate(() => {
    const w = window as any
    w.resumeAnimationFrame = window.requestAnimationFrame
    w.heldFrames = []
    window.requestAnimationFrame = callback => { w.heldFrames.push(callback); return 0 }
  })
  // Drain any callback scheduled before the pause; new app frames are held.
  await page.waitForTimeout(100)
  try {
    await MobileEmbed.swipe(page, { x: 330, y: 420 }, { x: 330, y: 200 }, 0)
    await expect.poll(() => page.evaluate(() => scrollY)).toBeGreaterThan(50)
  } finally {
    await page.evaluate(() => {
      const w = window as any
      window.requestAnimationFrame = w.resumeAnimationFrame
      for (const callback of w.heldFrames) requestAnimationFrame(callback)
    })
  }
})

test('mobile vertical wire-to-wire gate drag captures the touch without page scroll', async ({ page }) => {
  await MobileEmbed.allowPageScrollInBothDirections(page)
  await MobileEmbed.swipe(page, { x: 88, y: 424 }, { x: 88, y: 312 })
  await expect.poll(() => page.evaluate(() => ({
    cols: JSON.parse((window as any).runner.circuitJSON()).cols, scrollY,
  }))).toEqual({ cols: [['H', 'X'], ...Array.from({ length: 13 }, () => ['X'])], scrollY: 400 })
})

test('mobile vertical palette drag captures the touch without page scroll', async ({ page }) => {
  await MobileEmbed.allowPageScrollInBothDirections(page)
  await MobileEmbed.swipe(page, { x: 171, y: 148 }, { x: 200, y: 368 })
  await expect.poll(() => page.evaluate(() => ({
    cols: JSON.parse((window as any).runner.circuitJSON()).cols, scrollY,
  }))).toEqual({ cols: [['H', 1, 1, 'X'], ['X'], ['X', 1, 'H'], ...Array.from({ length: 11 }, () => ['X'])], scrollY: 400 })
})


test('mobile state panel header drag captures touch without scrolling the article', async ({ page }, testInfo) => {
  await MobileEmbed.start(page, circuit, ['H', 'X'], true)
  await page.evaluate(async () => {
    document.querySelector('canvas')!.style.height = '800px'
    for (let i = 0; i < 4; i++) await new Promise(requestAnimationFrame)
  })
  const before = await page.evaluate(() => (window as any).runner.circuitJSON())
  await MobileEmbed.allowPageScrollInBothDirections(page)
  await page.screenshot({ path: testInfo.outputPath('state-panel-before.png') })
  await MobileEmbed.swipe(page, { x: 150, y: 592 }, { x: 150, y: 492 })
  await page.screenshot({ path: testInfo.outputPath('state-panel-after.png') })
  expect(await page.evaluate(() => ({ circuit: (window as any).runner.circuitJSON(), scrollY })))
    .toEqual({ circuit: before, scrollY: 400 })
})

test('mobile editable block label vertical drag removes the block without page scroll', async ({ page }) => {
  const cols = JSON.parse(circuit).cols
  const blocked = JSON.stringify({ cols: [['{comment'], cols[0], ['}'], ...cols.slice(1)] })
  await MobileEmbed.start(page, blocked, ['H', 'X', '{}'])
  await MobileEmbed.allowPageScrollInBothDirections(page)
  await MobileEmbed.swipe(page, { x: 88, y: 242 }, { x: 88, y: 142 })
  await expect.poll(() => page.evaluate(() => ({
    cols: JSON.parse((window as any).runner.circuitJSON()).cols, scrollY,
  }))).toEqual({ cols, scrollY: 400 })
})
