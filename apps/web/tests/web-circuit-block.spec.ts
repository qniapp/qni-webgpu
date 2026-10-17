import { expect, test, type Page } from '@playwright/test'
import {
  dragPointer,
  getPaletteGateCenter,
  UI_CONSTANTS,
  waitForStartupReady,
  waitForValue,
} from './support/web-spec-helpers'

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
  await page.mouse.click(170, 280) // ブロックの下で、既存の比較画像と同じ列 0 を固定する。
  await page.mouse.move(1000, 450)

  // 測定結果は確率的に決まるため、測定ゲートの手前までを撮る。
  await expect(page).toHaveScreenshot('circuit-block-bell.png', {
    clip: { x: 120, y: 228, width: 200, height: 244 },
  })
})

// コメントブロックの編集。パレットの右下の項目 (通し番号 36) を回路へ運ぶ。
const COMMENT_BLOCK_PALETTE_INDEX = 36
const EGUI_PANEL_MARGIN = 8
const BELL_JSON = '{"cols":[["|0>","|0>"],["H"],["•","X"]]}'
const BLOCK_JSON = '{"cols":[["|0>","|0>"],["{量子もつれ"],["H"],["}"],["•","X"]]}'

type Rect = { left: number, top: number, right: number, bottom: number }
type BlockGeometry = {
  blocks: { label: string, start: number, end: number, body: Rect, labels: Rect[], handles: Rect[] }[]
  editor: { index: number, draft: string } | null
  hovered: number | null
}

const center = (rect: Rect) => ({ x: (rect.left + rect.right) / 2, y: (rect.top + rect.bottom) / 2 })

const slot = (column: number) => ({
  x: EGUI_PANEL_MARGIN + UI_CONSTANTS.LINE_LEFT_OFFSET + UI_CONSTANTS.GATE_SIZE + UI_CONSTANTS.SLOT_SPACING * column,
  y: EGUI_PANEL_MARGIN + UI_CONSTANTS.LINE_Y,
})

const hashJson = (page: Page) => decodeURIComponent(new URL(page.url()).hash.slice(1))

const readGeometry = async (page: Page): Promise<BlockGeometry> =>
  JSON.parse(await page.evaluate(() => (window as any).__qniCircuitBlockGeometryJson ?? '{"blocks":[],"editor":null,"hovered":null}'))

const waitForGeometry = (page: Page, done: (geometry: BlockGeometry) => boolean, message: string) =>
  waitForValue(() => readGeometry(page), done, message)

const openCircuit = async (page: Page, json: string) => {
  await page.goto(`/#${encodeURIComponent(json)}`)
  await waitForStartupReady(page, { waitForStateVector: true })
}

// egui の文字入力は eframe の隠し <input> (text agent) が受ける。編集欄が開いた
// フレームの後でフォーカスが移るため、打鍵の前にフォーカスを待つ。
const waitForTextAgentFocus = (page: Page) =>
  page.waitForFunction(() => document.activeElement?.tagName === 'INPUT', null, { timeout: 15_000 })

const dropCommentBlock = async (page: Page, column: number) => {
  const box = await page.locator('#egui-canvas').boundingBox()
  if (!box) {
    throw new Error('expected egui canvas to be measurable')
  }
  await dragPointer(page, getPaletteGateCenter(box.width, COMMENT_BLOCK_PALETTE_INDEX), slot(column), 12)
  await waitForGeometry(page, (geometry) => geometry.editor !== null, 'the comment editor did not open after the drop')
  await waitForTextAgentFocus(page)
}

const openLabelEditor = async (page: Page) => {
  const geometry = await waitForGeometry(page, (seen) => seen.blocks.length === 1, 'the block was not laid out')
  const label = center(geometry.blocks[0].labels[0])
  await page.mouse.click(label.x, label.y)
  await waitForGeometry(page, (seen) => seen.editor !== null, 'the comment editor did not open after the click')
  await waitForTextAgentFocus(page)
}

test('dropping the palette comment block on the H column writes a block to the URL', async ({ page }) => {
  await openCircuit(page, BELL_JSON)
  await dropCommentBlock(page, 1)

  await expect.poll(() => hashJson(page)).toBe('{"cols":[["|0>","|0>"],["{コメント"],["H"],["}"],["•","X"]]}')
})

test('typing a comment after the drop replaces the default label', async ({ page }) => {
  await openCircuit(page, BELL_JSON)
  await dropCommentBlock(page, 1)
  await page.keyboard.type('量子もつれ')
  await page.keyboard.press('Enter')

  await expect.poll(() => hashJson(page)).toBe(BLOCK_JSON)
})

test('clicking a label and typing updates the comment in the URL', async ({ page }) => {
  await openCircuit(page, BLOCK_JSON)
  await openLabelEditor(page)
  await page.keyboard.type('ベル状態')
  await page.keyboard.press('Enter')

  await expect.poll(() => hashJson(page)).toBe('{"cols":[["|0>","|0>"],["{ベル状態"],["H"],["}"],["•","X"]]}')
})

test('Escape cancels the comment edit', async ({ page }) => {
  await openCircuit(page, BLOCK_JSON)
  await openLabelEditor(page)
  await page.keyboard.type('ベル状態')
  await page.keyboard.press('Escape')
  await waitForGeometry(page, (geometry) => geometry.editor === null, 'the comment editor did not close')

  expect(hashJson(page)).toBe(BLOCK_JSON)
})

test('Enter that confirms an IME conversion does not commit the comment', async ({ page }) => {
  await openCircuit(page, BLOCK_JSON)
  await openLabelEditor(page)
  const cdp = await page.context().newCDPSession(page)
  await cdp.send('Input.imeSetComposition', { text: 'べる', selectionStart: 2, selectionEnd: 2 })
  // 変換確定の Enter は Chrome では keyCode 229 の keydown として届く。
  await cdp.send('Input.dispatchKeyEvent', { type: 'rawKeyDown', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 229 })
  await cdp.send('Input.insertText', { text: 'ベル' })
  await cdp.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13 })
  await waitForGeometry(page, (geometry) => geometry.editor?.draft === 'ベル', 'the IME text did not reach the draft')

  expect(hashJson(page)).toBe(BLOCK_JSON)
})

test('dragging the right handle one column moves the closing marker one column right', async ({ page }) => {
  await openCircuit(page, BLOCK_JSON)
  const geometry = await waitForGeometry(page, (seen) => seen.blocks.length === 1, 'the block was not laid out')
  const handle = center(geometry.blocks[0].handles[1])
  await page.mouse.move(handle.x - 16, handle.y)
  await waitForGeometry(page, (seen) => seen.hovered === 0, 'hovering the block did not reveal its handles')
  await dragPointer(page, handle, { x: handle.x + UI_CONSTANTS.SLOT_SPACING, y: handle.y }, 8)

  await expect.poll(() => hashJson(page)).toBe('{"cols":[["|0>","|0>"],["{量子もつれ"],["H"],["•","X"],["}"]]}')
})

test('dragging a label out of the circuit removes the block but keeps its gates', async ({ page }) => {
  await openCircuit(page, BLOCK_JSON)
  const geometry = await waitForGeometry(page, (seen) => seen.blocks.length === 1, 'the block was not laid out')
  const label = center(geometry.blocks[0].labels[0])
  await dragPointer(page, label, { x: label.x, y: label.y - 80 }, 8)

  await expect.poll(() => hashJson(page)).toBe(BELL_JSON)
})

test('hovered circuit block shows its resize handles', async ({ page }) => {
  await openCircuit(page, BLOCK_JSON)
  const geometry = await waitForGeometry(page, (seen) => seen.blocks.length === 1, 'the block was not laid out')
  const body = geometry.blocks[0].body
  await page.mouse.move(body.right - 8, (body.top + body.bottom) / 2 + 20)
  await waitForGeometry(page, (seen) => seen.hovered === 0, 'hovering the block did not reveal its handles')
  await page.waitForTimeout(400) // ハンドルの出現アニメーション (250ms) を待つ。

  await expect(page).toHaveScreenshot('circuit-block-hover.png', {
    clip: { x: body.left - 40, y: body.top - 32, width: body.right - body.left + 80, height: body.bottom - body.top + 64 },
  })
})
