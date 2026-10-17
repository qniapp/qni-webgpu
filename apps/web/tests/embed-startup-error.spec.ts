import { test, expect, type Page } from '@playwright/test'
import { readFile } from 'node:fs/promises'
import path from 'node:path'

// Raw error details must not vary with the installed Chrome version.
test.use({ userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36' })

const assetOrigin = 'http://localhost:4175/tutorial/assets/'
const fontOrigin = 'http://localhost:4175/test-fonts/'
const titles = {
  'insecure-context': '安全な接続で開いてください',
  'wasm-unavailable': 'ブラウザーの設定で止められています',
  'webgpu-unsupported': 'このブラウザーでは動きません',
  'gpu-adapter-unavailable': 'GPU を使えませんでした',
  'gpu-device-unavailable': 'GPU の準備に失敗しました',
  'gpu-device-lost': 'GPU との接続が切れました',
  'load-failed': '読み込みに失敗しました',
  'startup-timeout': '起動が止まってしまいました',
  'invalid-embed': '回路のデータに問題があります',
  unknown: '量子回路を起動できませんでした',
} as const

type Code = keyof typeof titles
type Platform = { os: string, browser: string, version: number, mobile: boolean }
const desktop: Platform = { os: 'windows', browser: 'chrome', version: 140, mobile: false }
const android: Platform = { os: 'android', browser: 'chrome', version: 140, mobile: true }
const sizes = {
  wide: { viewport: { width: 1280, height: 800 }, width: 960, height: 480, platform: desktop },
  phone: { viewport: { width: 375, height: 812 }, width: 343, height: 520, platform: android },
  short: { viewport: { width: 1280, height: 800 }, width: 960, height: 200, platform: desktop },
}
type ErrorHost = {
  code?: Code, size?: keyof typeof sizes, platform?: Platform, message?: string,
  retry?: boolean, articleHeight?: number, headingLevel?: 2 | 3 | 4,
}

// All module requests use the actual build output, including the renderer.
async function routeBundle(page: Page, wasm404 = false) {
  await page.route(`${assetOrigin}*`, async route => {
    const file = new URL(route.request().url()).pathname.split('/').pop()!
    const wasm = file.endsWith('.wasm')
    await route.fulfill({
      status: wasm && wasm404 ? 404 : 200,
      body: wasm && wasm404 ? 'not found' : await readFile(path.resolve(__dirname, '../embed-dist', file)),
      contentType: wasm ? 'application/wasm' : 'text/javascript',
      headers: { 'Access-Control-Allow-Origin': '*' },
    })
  })
}

const slot = (page: Page) => page.locator('qni-error-test .slot')

async function hostError(page: Page, {
  code = 'unknown', size = 'wide', platform = sizes[size].platform,
  message = 'raw message', retry = true, articleHeight = 0, headingLevel = 3,
}: ErrorHost = {}) {
  const frame = sizes[size]
  await page.setViewportSize(frame.viewport)
  await routeBundle(page)
  await page.route(`${fontOrigin}*`, async route => {
    const file = new URL(route.request().url()).pathname.split('/').pop()!
    await route.fulfill({
      body: await readFile(path.resolve(__dirname, '../assets', file)),
      contentType: file.endsWith('.otf') ? 'font/otf' : 'font/ttf',
      headers: { 'Access-Control-Allow-Origin': '*' },
    })
  })
  await page.route('**/host/startup-error', route => route.fulfill({
    contentType: 'text/html',
    body: `<!doctype html><html><head><meta charset="utf-8"><style>
      @font-face { font-family: "Qni Test Geist"; src: url("${fontOrigin}Geist-Regular.ttf"); font-weight: 400; }
      @font-face { font-family: "Qni Test JP"; src: url("${fontOrigin}QniJapaneseFallback-Regular.otf"); font-weight: 400; }
      @font-face { font-family: "Qni Test Geist Mono"; src: url("${fontOrigin}GeistMono-Regular.ttf"); font-weight: 400; }
      :root { --qni-startup-error-font: "Qni Test Geist", "Qni Test JP"; --qni-startup-error-mono-font: "Qni Test Geist Mono", "Qni Test JP"; }
      * { color: red !important; font-size: 30px; }
      body { margin: 16px; }
      h1 { font-weight: 500; }
    </style></head><body><h1>Third-party tutorial</h1>
      <div style="height:${articleHeight}px"></div><qni-error-test></qni-error-test>
      <script type="module">
        try {
          const outer = document.querySelector('qni-error-test').attachShadow({ mode: 'open' });
          outer.innerHTML = '<style>:host{display:block;width:${frame.width}px;height:${frame.height}px}.slot{width:100%;height:100%}*{color:red!important;font-size:30px}</style><div class="slot"></div>';
          const container = outer.querySelector('.slot');
          const m = await import('${assetOrigin}qni-embed-error.mjs');
          window.retried = 0;
          window.renderAgain = () => {
            window.handle = m.renderStartupError(container, new m.QniStartupError(${JSON.stringify(code)}, ${JSON.stringify(message)}), {
              platform: ${JSON.stringify(platform)}, headingLevel: ${headingLevel},
              onRetry: ${retry ? '() => { window.retried++; }' : 'undefined'},
            });
          };
          window.renderAgain();
          await document.fonts.load('16px "Qni Test Geist"');
          await document.fonts.load('16px "Qni Test JP"');
          await document.fonts.load('16px "Qni Test Geist Mono"');
          await document.fonts.ready;
          window.ready = true;
        } catch (error) { window.hostFailure = String(error); }
      </script></body></html>`,
  }))
  await page.goto('/host/startup-error')
  await page.waitForFunction(() => (window as any).ready || (window as any).hostFailure)
  const failure = await page.evaluate(() => (window as any).hostFailure)
  if (failure) throw new Error(failure)
}

for (const code of Object.keys(titles) as Code[]) {
  for (const size of ['wide', 'phone'] as const) {
    test(`startup error ${code} ${size}`, async ({ page }) => {
      await hostError(page, { code, size })
      await expect(slot(page)).toHaveScreenshot(`embed-error-${code}-${size}.png`)
    })
  }
}

const platformVariants: { name: string, code: Code, size: 'phone' | 'wide', platform: Platform }[] = [
  { name: 'webgpu-unsupported-ios-phone', code: 'webgpu-unsupported', size: 'phone', platform: { os: 'ios', browser: 'safari', version: 18, mobile: true } },
  { name: 'webgpu-unsupported-linux-wide', code: 'webgpu-unsupported', size: 'wide', platform: { os: 'linux', browser: 'chrome', version: 140, mobile: false } },
  { name: 'gpu-adapter-unavailable-firefox-wide', code: 'gpu-adapter-unavailable', size: 'wide', platform: { os: 'windows', browser: 'firefox', version: 141, mobile: false } },
]
for (const variant of platformVariants) {
  test(`startup error ${variant.name}`, async ({ page }) => {
    await hostError(page, variant)
    await expect(slot(page)).toHaveScreenshot(`embed-error-${variant.name}.png`)
  })
}

test('startup error adapter short frame', async ({ page }) => {
  await hostError(page, { code: 'gpu-adapter-unavailable', size: 'short' })
  await expect(slot(page)).toHaveScreenshot('embed-error-gpu-adapter-unavailable-short.png')
})

test('startup error unknown details open phone', async ({ page }) => {
  await hostError(page, { size: 'phone' })
  await slot(page).getByText('エラーの詳細', { exact: true }).click()
  await expect(slot(page)).toHaveScreenshot('embed-error-unknown-details-open-phone.png')
})

for (const [code, title] of Object.entries(titles)) {
  test(`${code} renders its Japanese title`, async ({ page }) => {
    await hostError(page, { code: code as Code })
    await expect(slot(page).getByRole('heading')).toHaveText(title)
  })
}

test('retry button calls onRetry once', async ({ page }) => {
  await hostError(page)
  await slot(page).getByRole('button', { name: '再試行', exact: true }).click()
  expect(await page.evaluate(() => (window as any).retried)).toBe(1)
})

test('invalid-embed has no retry button', async ({ page }) => {
  await hostError(page, { code: 'invalid-embed' })
  await expect(slot(page).getByRole('button', { name: '再試行', exact: true })).toHaveCount(0)
})

test('startup-timeout shows a reload button', async ({ page }) => {
  await hostError(page, { code: 'startup-timeout' })
  await expect(slot(page).getByRole('button', { name: 'ページを再読み込み', exact: true })).toBeVisible()
})

test('missing onRetry falls back to reload', async ({ page }) => {
  await hostError(page, { retry: false })
  await expect(slot(page).getByRole('button', { name: 'ページを再読み込み', exact: true })).toBeVisible()
})

test('missing onRetry updates the instructions to match reload', async ({ page }) => {
  await hostError(page, { retry: false })
  await expect(slot(page).locator('.steps li').first()).toHaveText('「ページを再読み込み」を押してください。')
})

test('retry is disabled while the host restarts', async ({ page }) => {
  await hostError(page)
  const button = slot(page).getByRole('button', { name: '再試行', exact: true })
  await button.click()
  await expect(button).toBeDisabled()
})

test('remove deletes the rendered host', async ({ page }) => {
  await hostError(page)
  await page.evaluate(() => (window as any).handle.remove())
  await expect(slot(page).locator('[data-qni-startup-error]')).toHaveCount(0)
})

test('naturalHeight includes the panel and outer padding', async ({ page }) => {
  await hostError(page)
  const delta = await slot(page).locator('.panel').evaluate(panel => {
    const scroller = panel.parentElement!
    const style = getComputedStyle(scroller)
    return (window as any).handle.naturalHeight() - panel.scrollHeight - parseFloat(style.paddingTop) - parseFloat(style.paddingBottom)
  })
  expect(delta).toBe(0)
})

for (const headingLevel of [2, 3, 4] as const) {
  test(`headingLevel ${headingLevel} controls the semantic heading`, async ({ page }) => {
    await hostError(page, { headingLevel })
    await expect(slot(page).getByRole('heading', { level: headingLevel })).toHaveText(titles.unknown)
  })
}

test('copy failure tells the user', async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, 'clipboard', { value: { writeText: async () => { throw new Error('denied') } } })
  })
  await hostError(page)
  await slot(page).getByText('エラーの詳細', { exact: true }).click()
  await slot(page).getByRole('button', { name: '詳細をコピー', exact: true }).click()
  await expect(slot(page).getByRole('button', { name: 'コピーできませんでした', exact: true })).toBeVisible()
})

test('URL copy failure selects the URL instead of diagnostic details', async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, 'clipboard', { value: { writeText: async () => { throw new Error('denied') } } })
  })
  await hostError(page, { code: 'webgpu-unsupported' })
  await slot(page).getByRole('button', { name: 'このページの URL をコピー', exact: true }).click()
  await slot(page).getByRole('button', { name: 'コピーできませんでした', exact: true }).waitFor()
  expect(await page.evaluate(() => getSelection()?.toString())).toBe('http://127.0.0.1:4174/host/startup-error')
})

test('error details show the raw message', async ({ page }) => {
  await hostError(page)
  await slot(page).getByText('エラーの詳細', { exact: true }).click()
  await expect(slot(page).locator('pre')).toContainText('raw message')
})

test('copy details writes the details to the clipboard', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write'])
  await hostError(page)
  await slot(page).getByText('エラーの詳細', { exact: true }).click()
  await slot(page).getByRole('button', { name: '詳細をコピー', exact: true }).click()
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toContain('code: unknown')
})

test('host CSS does not leak into the error screen', async ({ page }) => {
  await hostError(page)
  await expect(slot(page).getByRole('heading')).toHaveCSS('color', 'rgb(16, 15, 15)')
})

test('error screen CSS does not leak into the host', async ({ page }) => {
  await hostError(page)
  await expect(page.locator('h1')).toHaveCSS('font-weight', '500')
})

test('short frame scrolls inside instead of overflowing', async ({ page }) => {
  await hostError(page, { code: 'gpu-adapter-unavailable', size: 'short' })
  expect(await slot(page).locator('.scroller').evaluate(el => el.scrollHeight > el.clientHeight)).toBe(true)
})

test('wide frame uses the full illustration despite host font size', async ({ page }) => {
  await hostError(page)
  await expect(slot(page).locator('.art')).toHaveCSS('width', '160px')
})

test('narrow frame stacks the illustration with the heading', async ({ page }) => {
  await hostError(page, { size: 'phone' })
  await expect(slot(page).locator('.art')).toHaveCSS('width', '64px')
})

test('reduced motion disables the crayon animation', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' })
  await hostError(page)
  await expect(slot(page).locator('.draw').first()).toHaveCSS('animation-name', 'none')
})

test('status region announces the title', async ({ page }) => {
  await hostError(page)
  await expect(slot(page).getByRole('status')).toContainText(titles.unknown)
})

test('re-render after a focused retry moves focus to the heading', async ({ page }) => {
  await hostError(page)
  await slot(page).getByRole('button', { name: '再試行', exact: true }).focus()
  await page.evaluate(() => (window as any).renderAgain())
  await expect(slot(page).getByRole('heading')).toBeFocused()
})

test('rendering never moves the host page scroll', async ({ page }) => {
  await hostError(page, { articleHeight: 2000 })
  expect(await page.evaluate(() => scrollY)).toBe(0)
})

// Separate real host: startEmbed and its generated wasm bindings are never replaced.
async function hostRealEmbed(page: Page, { circuit = '{"cols":[["H"]]}', wasm404 = false } = {}) {
  await routeBundle(page, wasm404)
  await page.route('**/host/real-startup-error', route => route.fulfill({
    contentType: 'text/html',
    body: `<!doctype html><html><head><meta charset="utf-8"></head><body>
      <qni-real-error-test></qni-real-error-test><script type="module">
        try {
          const outer = document.querySelector('qni-real-error-test').attachShadow({ mode: 'open' });
          outer.innerHTML = '<style>:host{display:block;width:960px;height:640px}canvas{display:block;width:100%;height:100%}</style><canvas></canvas>';
          const { startEmbed } = await import('${assetOrigin}qni-embed.mjs');
          window.runner = await startEmbed(outer.querySelector('canvas'), ${JSON.stringify(circuit)});
          window.started = true;
        } catch (error) {
          window.startError = String(error);
          window.startErrorCode = error.code;
        }
      </script></body></html>`,
  }))
  await page.goto('/host/real-startup-error')
  await page.waitForFunction(() => (window as any).started || (window as any).startError)
}

async function removeGpu(page: Page) {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, 'gpu', { configurable: true, value: undefined })
  })
}

test('real startEmbed without navigator.gpu rejects as webgpu-unsupported', async ({ page }) => {
  await removeGpu(page)
  await hostRealEmbed(page)
  expect(await page.evaluate(() => (window as any).startErrorCode)).toBe('webgpu-unsupported')
})

test('real startEmbed without navigator.gpu does not request wasm', async ({ page }) => {
  await removeGpu(page)
  let wasmRequests = 0
  page.on('request', request => {
    if (request.url().startsWith(assetOrigin) && new URL(request.url()).pathname.endsWith('.wasm')) wasmRequests++
  })
  await hostRealEmbed(page)
  expect(wasmRequests).toBe(0)
})

test('real null adapter rejects as gpu-adapter-unavailable', async ({ page }) => {
  await page.addInitScript(() => { GPU.prototype.requestAdapter = async () => null })
  await hostRealEmbed(page)
  expect(await page.evaluate(() => (window as any).startErrorCode)).toBe('gpu-adapter-unavailable')
})

test('real device failure rejects as gpu-device-unavailable', async ({ page }) => {
  await page.addInitScript(() => {
    GPUAdapter.prototype.requestDevice = () => Promise.reject(new Error('injected device failure'))
  })
  await hostRealEmbed(page)
  expect(await page.evaluate(() => (window as any).startErrorCode)).toBe('gpu-device-unavailable')
})

test('real wasm 404 rejects as load-failed', async ({ page }) => {
  await hostRealEmbed(page, { wasm404: true })
  expect(await page.evaluate(() => (window as any).startErrorCode)).toBe('load-failed')
})

test('real invalid circuit rejects as invalid-embed', async ({ page }) => {
  await hostRealEmbed(page, { circuit: '{' })
  expect(await page.evaluate(() => (window as any).startErrorCode)).toBe('invalid-embed')
})
