import { expect, test } from '@playwright/test'
import { waitForStartupReady } from './support/web-spec-helpers'

test.use({ viewport: { width: 1280, height: 800 } })

// qni のトップページと同じ、ラベル付きブロックで囲んだベル状態の回路。
const BELL_BLOCK_JSON =
  '{"cols":[["|0>","|0>"],["{量子もつれ"],["H"],["•","X"],["}"],["Measure"],[1,"Measure"]]}'

test('circuit block markers survive loading from the URL', async ({ page }) => {
  await page.goto(`/#${encodeURIComponent(BELL_BLOCK_JSON)}`)
  await waitForStartupReady(page, { waitForStateVector: true })

  expect(decodeURIComponent(new URL(page.url()).hash.slice(1))).toBe(BELL_BLOCK_JSON)
})

test('circuit block visual snapshot', async ({ page }) => {
  await page.goto(`/#${encodeURIComponent(BELL_BLOCK_JSON)}`)
  await waitForStartupReady(page, { waitForStateVector: true })

  // 測定結果は確率的に決まるため、測定ゲートの手前までを撮る。
  await expect(page).toHaveScreenshot('circuit-block-bell.png', {
    clip: { x: 120, y: 200, width: 200, height: 190 },
  })
})
