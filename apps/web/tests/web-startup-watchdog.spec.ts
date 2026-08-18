import { expect, test } from '@playwright/test'

// WebGPU の初期化は例外も出さずに固まることがある。実測では 3 並列 / 4 CPU で
// 60 回の読み込みのうち 2 回が 90 秒たっても描画へ到達せず、残りは 1.4 秒以内に
// 描画できた。遅いのではなく固まっているので、読み込み直すのが唯一の回復手段。
// ここでは `navigator.gpu.requestAdapter` を差し替えて固まり方を再現し、監視の
// 期限は `__qniStartupWatchdogMs` で短縮する。
const WATCHDOG_MS = 1_000

test('startup that stalls once recovers on the automatic reload', async ({ page }) => {
  await page.addInitScript((watchdogMs: number) => {
    Reflect.set(window, '__qniStartupWatchdogMs', watchdogMs)
    if (!navigator.gpu) return
    // 1 回目の読み込みだけ固める。自動再読み込み後は本来のアダプタ取得に戻す。
    if (sessionStorage.getItem('qniStartupRetry') !== null) return
    Object.defineProperty(navigator.gpu, 'requestAdapter', {
      configurable: true,
      value: () => new Promise(() => {}),
    })
  }, WATCHDOG_MS)

  await page.goto('/')

  await page.waitForFunction(() => window.__eguiReady === true, null, { timeout: 30_000 })

  await expect(page.getByTestId('webgpu-error')).toBeHidden()
})

test('startup that keeps stalling surfaces the error after the reload', async ({ page }) => {
  await page.addInitScript((watchdogMs: number) => {
    Reflect.set(window, '__qniStartupWatchdogMs', watchdogMs)
    if (!navigator.gpu) return
    Object.defineProperty(navigator.gpu, 'requestAdapter', {
      configurable: true,
      value: () => new Promise(() => {}),
    })
  }, WATCHDOG_MS)

  await page.goto('/')

  await expect(page.getByTestId('webgpu-error')).toContainText('WebGPU initialization', {
    timeout: 30_000,
  })
})

// 監視が先に発火しても、そのあとで起動が完了すれば起動側が正しい。
// エラーフラグを取り消さないと、動いているのに異常として見えてしまう。
test('slow but successful startup clears the watchdog error', async ({ page }) => {
  await page.addInitScript((watchdogMs: number) => {
    Reflect.set(window, '__qniStartupWatchdogMs', watchdogMs)
    // 自動再読み込みを挟まずに監視の発火だけを再現するため、印を先に置く。
    sessionStorage.setItem('qniStartupRetry', 'test')
    if (!navigator.gpu) return
    const requestAdapter = navigator.gpu.requestAdapter.bind(navigator.gpu)
    Object.defineProperty(navigator.gpu, 'requestAdapter', {
      configurable: true,
      value: async (options?: GPURequestAdapterOptions) => {
        await new Promise((resolve) => setTimeout(resolve, watchdogMs * 3))
        return requestAdapter(options)
      },
    })
  }, WATCHDOG_MS)

  await page.goto('/')
  await page.waitForFunction(() => window.__eguiReady === true, null, { timeout: 30_000 })

  const startupError = await page.evaluate(() => window.__eguiError ?? null)

  expect(startupError).toBeNull()
})
