import { expect, test } from '@playwright/test'

// WebGPU の初期化は例外も出さずに固まることがある (Chrome を並列に多数起動した
// ときに実測)。何も出ないと利用者にはキャンバスが白いまま見えるため、期限を
// 過ぎたら明示的なエラー表示に切り替える。
// ここでは `navigator.gpu.requestAdapter` を差し替えて固まり方を再現し、
// 監視の期限は `__qniStartupWatchdogMs` で短縮する。
const WATCHDOG_MS = 1_000

test('stalled WebGPU initialization surfaces the startup error', async ({ page }) => {
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
// エラー表示とエラーフラグの両方を取り消さないと、動いているのにエラーが
// 残ったままになり、テストからも異常として見えてしまう。
test('slow but successful startup clears the watchdog error', async ({ page }) => {
  await page.addInitScript((watchdogMs: number) => {
    Reflect.set(window, '__qniStartupWatchdogMs', watchdogMs)
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
