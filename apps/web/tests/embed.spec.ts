import { test, expect, type Page } from '@playwright/test'
import { readFile } from 'node:fs/promises'
import path from 'node:path'

// Serve the bundle on a different origin and nested path without a second server.
const assetOrigin = 'http://localhost:4175/tutorial/assets/'
const CNOT = '{"cols":[["|0>","|0>"],["H"],["•","X"]]}'
const BB84 = '{"cols":[["{送信内容を決める2つの乱数を生成"],["|0>"],["H"],["Measure>aliceX"],["|0>"],["H"],["Measure>aliceH"],["}"],["|0>"],["{|1⟩をセット"],["X<aliceX"],["}"],["Bloch"],["{Hを適用"],["H<aliceH"],["}"],["Bloch"],["Swap","Swap"],["{🕶イブ"],[1,"Measure>eveX"],[1,"|0>"],[1,"X<eveX"],[1,"Bloch"],["}"],[1],["{Hのための乱数を生成"],[1,1,"|0>"],[1,1,"H"],[1,1,"Measure>bobH"],["}"],[1,"Swap","Swap"],["{Hを適用"],[1,1,"H<bobH"],["}"],[1,1,"Bloch"],["{測定"],[1,1,"Measure"],["}"],[1]]}'
type EmbedHost = { settings?: { showStatePanel?: boolean, palette?: string[], maxWireCount?: number }, width?: number, height?: number }

async function hostEmbed(page: Page, circuit = '{"cols":[["H"]]}', {
  settings = { showStatePanel: true }, width = 960, height = 640,
}: EmbedHost = {}) {
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
    body: `<html><head><meta charset="utf-8"></head><body style="margin:0"><h1>Third-party tutorial</h1>
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
          shadow.innerHTML = '<style>:host{display:block;width:${width}px;height:${height}px}canvas{width:100%;height:100%;display:block}</style><canvas></canvas>';
        }
      });
      try {
        const {startEmbed} = await import('${assetOrigin}qni-embed.mjs');
        window.startEmbed = startEmbed;
        window.canvas = document.querySelector('qni-webgpu-test').shadowRoot.querySelector('canvas');
        window.runner = await startEmbed(window.canvas, ${JSON.stringify(circuit)}, ${JSON.stringify(settings)});
        window.readState = (await import('${assetOrigin}qni-web.js')).read_state_vector;
        window.started = true;
      } catch(error) { window.startError = String(error); }
      </script></body></html>`,
  }))
  await page.goto('/host/article?mode=gpu#host-fragment')
  await page.waitForFunction(() => (window as any).started || (window as any).startError)
}

// Every circuit used here starts in |0...0> at step 0.
async function waitForStepZero(page: Page) {
  await page.waitForFunction(async () => {
    try { return (await (window as any).readState())[0] === 1 } catch { return false }
  })
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

test('qni BB84 tutorial circuit with measurement variables starts', async ({ page }) => {
  // Issue #53: qni `bb84_circuit.html` stores measurements in named variables
  // (`Measure>aliceX`) and conditions gates on them (`X<aliceX`).
  await hostEmbed(page, BB84, { settings: { showStatePanel: true, palette: [] } })
  expect(await page.evaluate(() => (window as any).startError)).toBeUndefined()
})

test('embed starts at step 0 like qni tutorials', async ({ page }) => {
  await hostEmbed(page, CNOT)
  await expect.poll(async () => Array.from(await page.evaluate(() => (window as any).readState()) as number[]))
    .toEqual([1, 0, 0, 0, 0, 0, 0, 0])
})

test('unknown palette gate rejects before runner startup', async ({ page }) => {
  await hostEmbed(page, CNOT, { settings: { palette: ['H', 'Foo'] } })
  expect(await page.evaluate(() => (window as any).startError)).toContain('unknown palette gate: Foo')
})

test('restricted palette gate drops into the lifted circuit', async ({ page }) => {
  await hostEmbed(page, '{"cols":[]}', { settings: { palette: ['X'] } })
  await waitForStepZero(page)
  // The one-gate palette is centred at x = 480 on the 960px canvas; its row
  // starts at PALETTE_ROW_Y = 80. A one-row palette lifts q0 by 48px from
  // 8 + LINE_Y (256), and slot 0 sits at LINE_LEFT_OFFSET + GATE_SIZE = 162.
  const canvas = page.locator('qni-webgpu-test canvas')
  const box = (await canvas.boundingBox())!
  await page.mouse.move(box.x + 480, box.y + 100)
  await page.mouse.down()
  await page.mouse.move(box.x + 300, box.y + 180, { steps: 8 })
  await page.mouse.move(box.x + 162, box.y + 216, { steps: 8 })
  await page.mouse.up()
  await expect.poll(async () => Array.from(await page.evaluate(() => (window as any).readState()) as number[]))
    .toEqual([0, 0, 1, 0])
})

test('small embed with full palette keeps CNOT clear of the state panel', async ({ page }) => {
  await hostEmbed(page, CNOT, { width: 960, height: 560 })
  await waitForStepZero(page)
  await expect(page.locator('qni-webgpu-test canvas')).toHaveScreenshot('embed-cnot-960x560-full-palette.png')
})

test('small embed with restricted palette keeps CNOT clear of the state panel', async ({ page }) => {
  await hostEmbed(page, CNOT, { settings: { palette: ['H', '•', 'X'] }, width: 960, height: 560 })
  await waitForStepZero(page)
  await expect(page.locator('qni-webgpu-test canvas')).toHaveScreenshot('embed-cnot-960x560-restricted-palette.png')
})

// qni tutorial pages at 390px give embeds a 354px canvas (issue #52).
const PHASE_PALETTE = ['H', 'X', 'Y', 'Z', 'P(π/2)', 'X^½', 'Rx(π/2)', 'Ry(π/2)', 'Rz(π/2)']
const ENTANGLEMENT = '{"cols":[["|0>","|0>"],["{量子もつれ"],["H"],["•","X"],["}"],["Measure"],[1,"Measure"]]}'
const ROTATIONS = '{"cols":[["|0>"],["X"],["Rx(π/2)"],["Rz(π/2)"],["Ry(π/2)"]]}'

test('narrow embed wraps a nine-gate palette into two rows', async ({ page }) => {
  await hostEmbed(page, '{"cols":[["|0>"]]}', { settings: { palette: PHASE_PALETTE }, width: 354, height: 592 })
  await waitForStepZero(page)
  await expect(page.locator('qni-webgpu-test canvas')).toHaveScreenshot('embed-narrow-phase-palette.png')
})

test('narrow embed keeps trailing measurements inside the canvas', async ({ page }) => {
  await hostEmbed(page, ENTANGLEMENT, { settings: { palette: ['H', '•', 'X'] }, width: 354, height: 592 })
  await waitForStepZero(page)
  await expect(page.locator('qni-webgpu-test canvas')).toHaveScreenshot('embed-narrow-entanglement.png')
})

test('narrow embed keeps five rotation columns inside the canvas', async ({ page }) => {
  await hostEmbed(page, ROTATIONS, { settings: { palette: ['Bloch'] }, width: 354, height: 592 })
  await waitForStepZero(page)
  await expect(page.locator('qni-webgpu-test canvas')).toHaveScreenshot('embed-narrow-rotations.png')
})

test('narrow embed drops a gate from the wrapped second palette row', async ({ page }) => {
  await hostEmbed(page, '{"cols":[]}', { settings: { palette: PHASE_PALETTE }, width: 354, height: 592 })
  await waitForStepZero(page)
  // Inside the central panel's 8px margin the 338px circuit area centres the
  // 5 + 4 palette (232px wide) at x = 8 + 53; Rx(π/2) is row 2, column 2,
  // whose centre is (8 + 53 + 48 + 20, 8 + 80 + 48 + 20). Two palette rows
  // keep q0 at 8 + LINE_Y (264); the compact gutter puts slot 0 at
  // 8 + 162 - 82 = 88.
  const canvas = page.locator('qni-webgpu-test canvas')
  const box = (await canvas.boundingBox())!
  await page.mouse.move(box.x + 129, box.y + 156)
  await page.mouse.down()
  await page.mouse.move(box.x + 110, box.y + 210, { steps: 8 })
  await page.mouse.move(box.x + 88, box.y + 264, { steps: 8 })
  await page.mouse.up()
  // Rx(π/2)|0> = (|0> - i|1>) / √2, read back as [re0, im0, re1, im1].
  await expect.poll(async () => Array.from(await page.evaluate(() => (window as any).readState()) as number[])
    .map(value => Math.round(value * 1000) / 1000))
    .toEqual([0.707, 0, 0, -0.707])
})

// qni's h_gate.html tutorial: data-max-wire-count="1" with an H/X palette.
const H_GATE_TUTORIAL = { settings: { palette: ['H', 'X'], maxWireCount: 1 } }
// The 88px H/X palette is centred at x = 480, so H sits at 480 - 24. The
// one-row palette lifts q0 to y = 216; slot 1 is 162 + SLOT_SPACING (56).
const PALETTE_H_X = 456
const SLOT_1_X = 218

// Embedded toolbar: Undo and Redo only, 32px buttons with an 8px gap inside
// the 12px / 6px toolbar padding. The clear button used to sit at x = 108.
const UNDO = { x: 28, y: 22 }
const REDO = { x: 68, y: 22 }
const FORMER_CLEAR = { x: 108, y: 22 }

const ZERO = '{"cols":[["|0>"]]}'
const ZERO_X = '{"cols":[["|0>"],["X"]]}'

async function dropXOnSlot1(page: Page) {
  const box = (await page.locator('qni-webgpu-test canvas').boundingBox())!
  await page.mouse.move(box.x + PALETTE_H_X + 48, box.y + 100)
  await page.mouse.down()
  await page.mouse.move(box.x + 300, box.y + 180, { steps: 8 })
  await page.mouse.move(box.x + SLOT_1_X, box.y + 216, { steps: 8 })
  await page.mouse.up()
}

async function clickToolbar(page: Page, position: { x: number, y: number }) {
  await page.locator('qni-webgpu-test canvas').click({ position })
}

async function waitForCircuit(page: Page, json: string) {
  await page.waitForFunction(json => (window as any).runner.circuitJSON() === json, json)
}

const circuitJSON = (page: Page) => page.evaluate(() => (window as any).runner.circuitJSON())

test('embed toolbar undo restores the circuit before an edit', async ({ page }) => {
  await hostEmbed(page, ZERO, H_GATE_TUTORIAL)
  await waitForStepZero(page)
  await dropXOnSlot1(page)
  await waitForCircuit(page, ZERO_X)
  await clickToolbar(page, UNDO)
  await expect.poll(() => circuitJSON(page)).toBe(ZERO)
})

test('embed toolbar redo reapplies the undone edit', async ({ page }) => {
  await hostEmbed(page, ZERO, H_GATE_TUTORIAL)
  await waitForStepZero(page)
  await dropXOnSlot1(page)
  await waitForCircuit(page, ZERO_X)
  await clickToolbar(page, UNDO)
  await waitForCircuit(page, ZERO)
  await clickToolbar(page, REDO)
  await expect.poll(() => circuitJSON(page)).toBe(ZERO_X)
})

test('embed toolbar has no clear button', async ({ page }) => {
  await hostEmbed(page, ZERO, H_GATE_TUTORIAL)
  await waitForStepZero(page)
  await clickToolbar(page, FORMER_CLEAR)
  // Events apply in order: had the click cleared |0>, X would land alone.
  await dropXOnSlot1(page)
  await expect.poll(() => circuitJSON(page)).toBe(ZERO_X)
})

test('embed edits, undo and redo never access host storage or history', async ({ page }) => {
  await hostEmbed(page, ZERO, H_GATE_TUTORIAL)
  await waitForStepZero(page)
  await dropXOnSlot1(page)
  await waitForCircuit(page, ZERO_X)
  await clickToolbar(page, UNDO)
  await waitForCircuit(page, ZERO)
  await clickToolbar(page, REDO)
  await waitForCircuit(page, ZERO_X)
  expect(await page.evaluate(() => ({ effects: (window as any).effects, url: location.pathname + location.search + location.hash })))
    .toEqual({ effects: [], url: '/host/article?mode=gpu#host-fragment' })
})

test('maxWireCount 1 draws a one-qubit circuit on one wire', async ({ page }) => {
  await hostEmbed(page, '{"cols":[["|0>"]]}', H_GATE_TUTORIAL)
  await waitForStepZero(page)
  await expect(page.locator('qni-webgpu-test canvas')).toHaveScreenshot('embed-max-wire-count-1.png')
})

test('maxWireCount 1 adds no wire while dragging', async ({ page }) => {
  await hostEmbed(page, '{"cols":[["|0>"]]}', H_GATE_TUTORIAL)
  await waitForStepZero(page)
  const canvas = page.locator('qni-webgpu-test canvas')
  const box = (await canvas.boundingBox())!
  await page.mouse.move(box.x + PALETTE_H_X, box.y + 100)
  await page.mouse.down()
  await page.mouse.move(box.x + 400, box.y + 330, { steps: 8 })
  await expect(canvas).toHaveScreenshot('embed-max-wire-count-1-drag.png')
})

test('maxWireCount 1 offers no second wire to drop on', async ({ page }) => {
  await hostEmbed(page, '{"cols":[["|0>"]]}', H_GATE_TUTORIAL)
  await waitForStepZero(page)
  const canvas = page.locator('qni-webgpu-test canvas')
  const box = (await canvas.boundingBox())!
  async function drag(fromX: number, toY: number) {
    await page.mouse.move(box.x + fromX, box.y + 100)
    await page.mouse.down()
    await page.mouse.move(box.x + 300, box.y + 240, { steps: 8 })
    await page.mouse.move(box.x + SLOT_1_X, box.y + toY, { steps: 8 })
    await page.mouse.up()
  }
  // H released where q1 would be is discarded; X then lands on q0. Without
  // the limit, H would stay on q1 and the column would read ["X","H"].
  await drag(PALETTE_H_X, 216 + 56)
  await drag(PALETTE_H_X + 48, 216)
  await expect.poll(() => page.evaluate(() => (window as any).runner.circuitJSON()))
    .toBe('{"cols":[["|0>"],["X"]]}')
})

// Issue #59: qni `superdense_coding_circuit.html` carries a root `title`.
const SUPERDENSE = '{"cols":[[1,1,1,"|0>","|0>"],["{ベル回路"],[1,1,1,"H"],[1,1,1,"•","X"],["}"],[1,1,"Swap","Swap"],[1,1,1,1,"Swap",1,"Swap"],["{送信する2ビットを生成"],["|0>","|0>"],["H","H"],["Measure>bit1","Measure>bit2"],["}"],[1],["|0>","|0>"],["{送信ビットに応じたベル状態を作る"],["X<bit1","X<bit2"],[1,"•","X"],["•",1,"Z"],["}"],["Measure","Measure"],[1,1,"Swap",1,1,"Swap"],["{ベル測定"],[1,1,1,1,1,"•","X"],[1,1,1,1,1,"H"],[1,1,1,1,1,"Measure","Measure"],["}"],[1]],"title":"Superdense Coding"}'

test('qni Superdense Coding tutorial circuit with a title starts', async ({ page }) => {
  await hostEmbed(page, SUPERDENSE, { settings: { showStatePanel: true, palette: [], maxWireCount: 1 } })
  expect(await page.evaluate(() => (window as any).startError)).toBeUndefined()
})

// Only a string `title` is accepted beside `cols`; anything else is rejected.
for (const [name, circuit] of [
  ['an unknown root key', '{"cols":[["H"]],"title":"Bell","mode":"gpu"}'],
  ['a null title', '{"cols":[["H"]],"title":null}'],
  ['a false title', '{"cols":[["H"]],"title":false}'],
  ['a zero title', '{"cols":[["H"]],"title":0}'],
  ['a number title', '{"cols":[["H"]],"title":123}'],
  ['an object title', '{"cols":[["H"]],"title":{"text":"Bell"}}'],
  ['an array title', '{"cols":[["H"]],"title":["Bell"]}'],
]) {
  test(`embed circuit with ${name} rejects before runner startup`, async ({ page }) => {
    await hostEmbed(page, circuit)
    expect(await page.evaluate(() => (window as any).startError)).toContain('invalid circuit JSON')
  })
}

test('embed keeps the trimmed circuit title in circuitJSON after an edit', async ({ page }) => {
  await hostEmbed(page, '{"title":" Superdense Coding ","cols":[["|0>"]]}', H_GATE_TUTORIAL)
  await waitForStepZero(page)
  const canvas = page.locator('qni-webgpu-test canvas')
  const box = (await canvas.boundingBox())!
  await page.mouse.move(box.x + PALETTE_H_X + 48, box.y + 100)
  await page.mouse.down()
  await page.mouse.move(box.x + 300, box.y + 180, { steps: 8 })
  await page.mouse.move(box.x + SLOT_1_X, box.y + 216, { steps: 8 })
  await page.mouse.up()
  await expect.poll(() => page.evaluate(() => (window as any).runner.circuitJSON()))
    .toBe('{"cols":[["|0>"],["X"]],"title":"Superdense Coding"}')
})

test('embed circuit title never renames the host page', async ({ page }) => {
  await hostEmbed(page, '{"cols":[["H"]],"title":"Bell"}')
  await waitForHState(page)
  expect(await page.title()).toBe('')
})

test('invalid maxWireCount rejects before runner startup', async ({ page }) => {
  await hostEmbed(page, '{"cols":[]}', { settings: { maxWireCount: 0 } })
  expect(await page.evaluate(() => (window as any).startError)).toContain('maxWireCount must be a positive integer')
})
