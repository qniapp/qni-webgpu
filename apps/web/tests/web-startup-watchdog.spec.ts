import { expect, test } from '@playwright/test'

// WebGPU の初期化は例外も出さずに固まることがある (Chrome + SwiftShader を
// 並列で多数起動したときに実測)。何も出ないと利用者にはキャンバスが白いまま
// 見えるため、期限を過ぎたら明示的なエラー表示に切り替える。
// ここでは `navigator.gpu.requestAdapter` を解決しない Promise に差し替えて
// 固まった状態を再現する。
test('stalled WebGPU initialization surfaces the startup error', async ({ page }) => {
  await page.addInitScript(() => {
    if (!navigator.gpu) return
    Object.defineProperty(navigator.gpu, 'requestAdapter', {
      configurable: true,
      value: () => new Promise(() => {}),
    })
  })

  await page.goto('/')

  await expect(page.getByTestId('webgpu-error')).toContainText('WebGPU initialization', {
    timeout: 30_000,
  })
})
