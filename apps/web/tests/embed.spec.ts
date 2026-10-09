import { test, expect, type Page } from '@playwright/test'
import { readFile } from 'node:fs/promises'
import path from 'node:path'

// Serve the bundle on a different origin and nested path without a second server.
const assetOrigin = 'http://localhost:4175/tutorial/assets/'
async function hostEmbed(page: Page, circuit = '{"cols":[["H"]]}') {
  await page.route(`${assetOrigin}*`, async route => {
    const file = new URL(route.request().url()).pathname.split('/').pop()!
    await route.fulfill({
      body: await readFile(path.resolve(__dirname, '../embed-dist', file)),
      contentType: file.endsWith('.wasm') ? 'application/wasm' : 'text/javascript',
      headers: { 'Access-Control-Allow-Origin': '*' },
    })
  })
  await page.route('**/host/article?mode=gpu', route => route.fulfill({
    contentType: 'text/html',
    body: `<html><body style="margin:0"><h1>Third-party tutorial</h1>
      <qni-webgpu-test></qni-webgpu-test><script type="module">
      window.effects = [];
      for (const name of ['pushState', 'replaceState']) {
        history[name] = () => { window.effects.push(name); throw Error(name); };
      }
      for (const name of ['getItem', 'setItem', 'removeItem', 'clear']) {
        Storage.prototype[name] = () => { window.effects.push(name); throw Error(name); };
      }
      customElements.define('qni-webgpu-test', class extends HTMLElement {
        constructor() {
          super();
          const shadow = this.attachShadow({mode: 'open'});
          shadow.innerHTML = '<style>:host{display:block;width:960px;height:640px}canvas{width:100%;height:100%;display:block}</style><canvas></canvas>';
        }
      });
      try {
        const {startEmbed} = await import('${assetOrigin}qni-embed.mjs');
        window.startEmbed = startEmbed;
        window.canvas = document.querySelector('qni-webgpu-test').shadowRoot.querySelector('canvas');
        window.runner = await startEmbed(window.canvas, ${JSON.stringify(circuit)}, {showStatePanel: true});
        window.readState = (await import('${assetOrigin}qni-web.js')).read_state_vector;
        window.started = true;
      } catch(error) { window.startError = String(error); }
      </script></body></html>`,
  }))
  await page.goto('/host/article?mode=gpu#host-fragment')
  await page.waitForFunction(() => (window as any).started || (window as any).startError)
}

async function waitForHState(page: Page) {
  await page.waitForFunction(async () => {
    try {
      const values = Array.from(await (window as any).readState()) as number[]
      return values.length === 4 && Math.abs(values[0] - Math.SQRT1_2) < 0.0001
        && Math.abs(values[2] - Math.SQRT1_2) < 0.0001
    } catch { return false }
  })
}

test('cross-origin bundle initializes H on a shadow DOM canvas', async ({ page }) => {
  await hostEmbed(page)
  await waitForHState(page)
  const values = await page.evaluate(async () => Array.from(await (window as any).readState()))
  expect(values).toEqual([expect.closeTo(Math.SQRT1_2, 5), 0, expect.closeTo(Math.SQRT1_2, 5), 0])
})

test('embed renders without library or external execution controls', async ({ page }) => {
  await hostEmbed(page)
  await waitForHState(page)
  await expect(page.locator('qni-webgpu-test canvas')).toHaveScreenshot('embed-h.png')
})

test('embed startup and clear never access host storage or history', async ({ page }) => {
  await hostEmbed(page)
  await waitForHState(page)
  // Embedded toolbar: Undo, Redo, Clear, with 32px buttons and 8px gaps.
  await page.locator('qni-webgpu-test canvas').click({ position: { x: 108, y: 22 } })
  await page.waitForFunction(async () => {
    try { return (await (window as any).readState())[0] === 1 } catch { return false }
  })
  expect(await page.evaluate(() => ({ effects: (window as any).effects, url: location.pathname + location.search + location.hash })))
    .toEqual({ effects: [], url: '/host/article?mode=gpu#host-fragment' })
})

test('destroy is idempotent and the same canvas can restart', async ({ page }) => {
  await hostEmbed(page)
  await waitForHState(page)
  await page.evaluate(async () => {
    const w = window as any
    w.runner.destroy()
    w.runner.destroy()
    w.runner = await w.startEmbed(w.canvas, '{"cols":[["X"]]}', {showStatePanel: true})
  })
  await page.waitForFunction(async () => {
    try { return (await (window as any).readState())[2] === 1 } catch { return false }
  })
  expect(await page.evaluate(async () => Array.from(await (window as any).readState())))
    .toEqual([0, 0, 1, 0])
})

test('invalid embed circuit rejects before runner startup', async ({ page }) => {
  await hostEmbed(page, '{"cols":[["unknown"]]}')
  expect(await page.evaluate(() => (window as any).startError)).toContain('invalid circuit JSON')
})
