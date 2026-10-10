import { expect, test, type Page } from '@playwright/test'
import { readStateVector, waitForStartupReady } from './support/web-spec-helpers'

test.use({ viewport: { width: 1280, height: 800 } })

// qni の測定変数 (`Measure>a`) と条件付きゲート (`X<a`)。
// 測定は q0 に書いた |1⟩ / |0⟩ を読むので結果が確定する。
const APPLIED_JSON = '{"cols":[["|1>","|0>"],["Measure>a"],[1,"X<a"]]}'
const SKIPPED_JSON = '{"cols":[["|0>","|0>"],["Measure>a"],[1,"X<a"]]}'
const UNSET_JSON = '{"cols":[["|0>","|0>"],[1,"X<b"]]}'
// 適用される条件付きゲート、一度も書かれない変数の条件付きゲート、
// 制御付き条件付きゲート (ラベルが下に回る) を 1 枚に収める。
const VISUAL_JSON = '{"cols":[["|1>","|0>"],["Measure>a"],[1,"X<a"],["H<b"],["•","Y<a"]]}'

// qni `apps/tutorial/bb84_circuit.html` (qniapp/qni@acf87bf) の回路。
const BB84_JSON =
  '{"cols":[["{送信内容を決める2つの乱数を生成"],["|0>"],["H"],["Measure>aliceX"],["|0>"],["H"],["Measure>aliceH"],["}"],["|0>"],["{|1⟩をセット"],["X<aliceX"],["}"],["Bloch"],["{Hを適用"],["H<aliceH"],["}"],["Bloch"],["Swap","Swap"],["{🕶イブ"],[1,"Measure>eveX"],[1,"|0>"],[1,"X<eveX"],[1,"Bloch"],["}"],[1],["{Hのための乱数を生成"],[1,1,"|0>"],[1,1,"H"],[1,1,"Measure>bobH"],["}"],[1,"Swap","Swap"],["{Hを適用"],[1,1,"H<bobH"],["}"],[1,1,"Bloch"],["{測定"],[1,1,"Measure"],["}"],[1]]}'

const openCircuit = async (page: Page, json: string): Promise<void> => {
  await page.goto(`/#${encodeURIComponent(json)}`)
  await waitForStartupReady(page, { waitForStateVector: true })
}

const roundedStateVector = async (page: Page): Promise<number[]> =>
  (await readStateVector(page) as number[]).map((value) => Math.round(value * 1e4) / 1e4)

const hashJson = (page: Page): string => decodeURIComponent(new URL(page.url()).hash.slice(1))

const waitForHashJson = async (page: Page, json: string): Promise<void> => {
  await page.waitForFunction(
    (expected) => decodeURIComponent(window.location.hash.slice(1)) === expected,
    json,
  )
}

test('conditional gate applies when its measurement reads 1', async ({ page }) => {
  await openCircuit(page, APPLIED_JSON)

  await expect.poll(() => roundedStateVector(page)).toEqual([0, 0, 0, 0, 0, 0, 1, 0])
})

test('conditional gate is skipped when its measurement reads 0', async ({ page }) => {
  await openCircuit(page, SKIPPED_JSON)

  await expect.poll(() => roundedStateVector(page)).toEqual([1, 0, 0, 0, 0, 0, 0, 0])
})

test('conditional gate on a never-measured variable is skipped', async ({ page }) => {
  await openCircuit(page, UNSET_JSON)

  await expect.poll(() => roundedStateVector(page)).toEqual([1, 0, 0, 0, 0, 0, 0, 0])
})

test('BB84 circuit keeps its measurement variables and conditions in the URL', async ({ page }) => {
  await openCircuit(page, BB84_JSON)

  // qni も保存時に末尾の空ステップを落とす。
  expect(hashJson(page)).toBe(BB84_JSON.replace(',[1]]}', ']}'))
})

test('shift copy keeps the condition of a conditional gate', async ({ page }) => {
  await openCircuit(page, APPLIED_JSON)
  await page.keyboard.down('Shift')
  await page.mouse.move(282, 320)
  await page.mouse.down()
  await page.mouse.move(310, 320, { steps: 4 })
  await page.mouse.move(338, 320, { steps: 4 })
  await page.mouse.up()
  await page.keyboard.up('Shift')

  await expect
    .poll(() => hashJson(page))
    .toBe('{"cols":[["|1>","|0>"],["Measure>a"],[1,"X<a"],[1,"X<a"]]}')
})

test('undo restores a deleted conditional gate with its condition', async ({ page }) => {
  await openCircuit(page, APPLIED_JSON)
  // 条件付きゲートを回路の外へドラッグして削除し、ツールバーの undo で戻す。
  await page.mouse.move(282, 320)
  await page.mouse.down()
  await page.mouse.move(282, 500, { steps: 8 })
  await page.mouse.move(282, 700, { steps: 8 })
  await page.mouse.up()
  await waitForHashJson(page, '{"cols":[["|1>","|0>"],["Measure>a"]]}')
  await page.mouse.click(124, 22)

  await expect.poll(() => hashJson(page)).toBe(APPLIED_JSON)
})

test('conditional gate visual snapshot', async ({ page }) => {
  await openCircuit(page, VISUAL_JSON)

  await expect(page).toHaveScreenshot('conditional-gates.png', {
    clip: { x: 120, y: 220, width: 320, height: 150 },
  })
})
