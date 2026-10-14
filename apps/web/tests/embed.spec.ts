import { test, expect, type Page } from '@playwright/test'
import { readFile } from 'node:fs/promises'
import path from 'node:path'

// Serve the bundle on a different origin and nested path without a second server.
const assetOrigin = 'http://localhost:4175/tutorial/assets/'
const CNOT = '{"cols":[["|0>","|0>"],["H"],["•","X"]]}'
const BB84 = '{"cols":[["{送信内容を決める2つの乱数を生成"],["|0>"],["H"],["Measure>aliceX"],["|0>"],["H"],["Measure>aliceH"],["}"],["|0>"],["{|1⟩をセット"],["X<aliceX"],["}"],["Bloch"],["{Hを適用"],["H<aliceH"],["}"],["Bloch"],["Swap","Swap"],["{🕶イブ"],[1,"Measure>eveX"],[1,"|0>"],[1,"X<eveX"],[1,"Bloch"],["}"],[1],["{Hのための乱数を生成"],[1,1,"|0>"],[1,1,"H"],[1,1,"Measure>bobH"],["}"],[1,"Swap","Swap"],["{Hを適用"],[1,1,"H<bobH"],["}"],[1,1,"Bloch"],["{測定"],[1,1,"Measure"],["}"],[1]]}'
type EmbedHost = {
  settings?: { showStatePanel?: boolean, palette?: string[], maxWireCount?: number, showRestartButton?: boolean },
  width?: number, height?: number, articleHeight?: number, fragment?: string,
}

async function hostEmbed(page: Page, circuit = '{"cols":[["H"]]}', {
  settings = { showStatePanel: true }, width = 960, height = 640, articleHeight = 0, fragment = '#host-fragment',
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
      <div style="height:${articleHeight}px"></div><qni-webgpu-test></qni-webgpu-test><script type="module">
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
        const settings = ${JSON.stringify(settings)};
        if (settings.showRestartButton) settings.onRestart = () => {
          window.restartCircuit = window.runner.circuitJSON();
          window.runner.destroy();
          startEmbed(window.canvas, window.restartCircuit, settings).then(runner => {
            window.runner = runner;
            window.restarted = true;
          });
        };
        window.runner = await startEmbed(window.canvas, ${JSON.stringify(circuit)}, settings);
        window.embedWasm = await import('${assetOrigin}qni-web.js');
        window.readState = window.embedWasm.read_state_vector;
        window.started = true;
      } catch(error) { window.startError = String(error); }
      </script></body></html>`,
  }))
  await page.goto(`/host/article?mode=gpu${fragment}`)
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

for (const failFirst of [false, true]) {
  test(`concurrent low-level embeds share ${failFirst ? 'a failed GPU initialization and retry' : 'the first GPU initialization'}`, async ({ page }) => {
    await page.addInitScript(failFirst => {
      const w = window as any
      w.deviceRequests = 0
      const prototype = GPUAdapter.prototype
      const requestDevice = prototype.requestDevice
      prototype.requestDevice = function (...args: any[]) {
        w.deviceRequests++
        if (failFirst && w.deviceRequests === 1) return Promise.reject(new Error('injected device failure'))
        return requestDevice.apply(this, args)
      }
    }, failFirst)
    await page.route(`${assetOrigin}*`, async route => {
      const file = new URL(route.request().url()).pathname.split('/').pop()!
      await route.fulfill({
        body: await readFile(path.resolve(__dirname, '../embed-dist', file)),
        contentType: file.endsWith('.wasm') ? 'application/wasm' : 'text/javascript',
        headers: { 'Access-Control-Allow-Origin': '*' },
      })
    })
    await page.route('**/host/concurrent', route => route.fulfill({
      contentType: 'text/html',
      body: '<html><body><canvas style="width:480px;height:640px"></canvas><canvas style="width:480px;height:640px"></canvas></body></html>',
    }))
    await page.goto('/host/concurrent')
    const result = await page.evaluate(async ({ origin, failFirst }) => {
      const { prepareEmbed } = await import(`${origin}qni-embed.mjs`)
      await prepareEmbed()
      const { start_embed } = await import(`${origin}qni-web.js`)
      const startBoth = () => Array.from(document.querySelectorAll('canvas'), canvas =>
        start_embed(canvas, '{"cols":[["H"]]}', true, undefined, undefined, false))
      let failure
      if (failFirst) {
        const attempts = await Promise.allSettled(startBoth())
        const errors = attempts.filter(attempt => attempt.status === 'rejected').map(attempt => attempt.reason)
        for (const attempt of attempts) {
          if (attempt.status === 'fulfilled') { attempt.value.destroy(); attempt.value.free() }
        }
        failure = { rejected: errors.length, sameError: errors.length === 2 && errors[0] === errors[1], deviceRequests: (window as any).deviceRequests }
      }
      const runners = await Promise.all(startBoth())
      try {
        for (const runner of runners) runner.selectStep(-1)
        const states = await Promise.all(runners.map(async runner => Array.from(await runner.read_state_vector())))
        return { runners: runners.length, states, deviceRequests: (window as any).deviceRequests, failure }
      } finally {
        for (const runner of runners) { runner.destroy(); runner.free() }
      }
    }, { origin: assetOrigin, failFirst })
    expect(result).toEqual({
      runners: 2, states: [[1, 0, 0, 0], [1, 0, 0, 0]], deviceRequests: failFirst ? 2 : 1,
      failure: failFirst ? { rejected: 2, sameError: true, deviceRequests: 1 } : undefined,
    })
  })
}

test('cross-origin bundle initializes H on a shadow DOM canvas', async ({ page }) => {
  await hostEmbed(page)
  await waitForHState(page)
  const values = await page.evaluate(async () => Array.from(await (window as any).readState()))
  expect(values).toEqual([expect.closeTo(Math.SQRT1_2, 5), 0, expect.closeTo(Math.SQRT1_2, 5), 0])
})

test('semantic step selection preserves preferredHeight', async ({ page }) => {
  await hostEmbed(page, CNOT)
  const result = await page.evaluate(() => {
    const runner = (window as any).runner
    const before = runner.preferredHeight(960)
    runner.selectStep(2)
    if (!Number.isFinite(before) || before <= 0) throw new Error('Invalid preferred height')
    return runner.preferredHeight(960) - before
  })
  expect(result).toBe(0)
})

test('preferredHeight rejects calls after runner destruction', async ({ page }) => {
  await hostEmbed(page, CNOT)
  const error = await page.evaluate(() => {
    const runner = (window as any).runner
    runner.destroy()
    try { runner.preferredHeight(960); return null } catch (error) { return String(error) }
  })
  expect(error).toContain('Circuit runner is destroyed')
})

test('embed renders without library or external execution controls', async ({ page }) => {
  await hostEmbed(page)
  await waitForHState(page)
  await expect(page.locator('qni-webgpu-test canvas')).toHaveScreenshot('embed-h.png')
})

test('opt-in restart icon appears at the right of the states label', async ({ page }) => {
  await hostEmbed(page, CNOT, { settings: { showRestartButton: true, palette: [] } })
  await waitForStepZero(page)
  await expect(page.locator('qni-webgpu-test canvas')).toHaveScreenshot('embed-restart.png')
})

for (const width of [354, 240]) {
  test(`narrow ${width}px eight-qubit embed keeps restart inside the frame`, async ({ page }) => {
    const circuit = JSON.stringify({ cols: [Array(8).fill('|0>')] })
    await hostEmbed(page, circuit, { width, height: 640, settings: { showRestartButton: true, palette: [], maxWireCount: 1 } })
    await waitForStepZero(page)
    await expect(page.locator('qni-webgpu-test canvas')).toHaveScreenshot(`embed-restart-narrow-${width}.png`)
  })
}

for (const action of ['pointer', 'Enter', 'Space'] as const) {
  test(`restart ${action} action lets host export, destroy and restart the real runner`, async ({ page }) => {
    await hostEmbed(page, CNOT, { settings: { showRestartButton: true, palette: [] } })
    await waitForStepZero(page)
    const canvas = page.locator('qni-webgpu-test canvas')
    const box = (await canvas.boundingBox())!
    // Default panel: centered 560px width, 160px viewport, 32px strip,
    // bottom margin 64px + content margin 8px. Icon center: 32px from right.
    const x = box.x + box.width / 2 + 280 - 32
    const y = box.y + box.height - 64 - 8 - 160 - 16
    if (action === 'pointer') await page.mouse.click(x, y)
    else {
      await canvas.focus()
      // Restart is the last registered focusable control. Reverse Tab enters
      // it directly without activating the aspect popover along the way.
      await page.keyboard.press('Shift+Tab')
      await page.waitForTimeout(100)
      await page.keyboard.press(action)
    }
    await page.waitForFunction(() => (window as any).restarted)
    expect(await page.evaluate(() => (window as any).restartCircuit)).toBe(CNOT)
  })
}

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

const SELECT_BELL = '{"cols":[["{Bell"],["H"],["•","X"],["}"]]}'
const bellStates = [
  [expect.closeTo(Math.SQRT1_2, 5), 0, expect.closeTo(Math.SQRT1_2, 5), 0, 0, 0, 0, 0],
  [expect.closeTo(Math.SQRT1_2, 5), 0, 0, 0, 0, 0, expect.closeTo(Math.SQRT1_2, 5), 0],
]

for (const showStatePanel of [true, false]) {
  for (const step of [0, 1]) {
    test(`selectStep ${step} immediately reads Bell state (panel=${showStatePanel})`, async ({ page }) => {
      await hostEmbed(page, SELECT_BELL, { settings: { showStatePanel } })
      const values = await page.evaluate(async n => {
        const runner = (window as any).runner
        // Switch away first, then read without polling or waiting for a frame.
        runner.selectStep(1 - n)
        runner.selectStep(n)
        return Array.from(await runner.readStateVector())
      }, step)
      expect(values).toEqual(bellStates[step])
    })
  }
}

test('low-level start_embed exposes the same step selection and readback', async ({ page }) => {
  await hostEmbed(page, SELECT_BELL)
  const values = await page.evaluate(async circuit => {
    const w = window as any
    w.runner.destroy()
    const runner = await w.embedWasm.start_embed(w.canvas, circuit, true, undefined, undefined, false)
    try {
      runner.selectStep(1)
      return Array.from(await runner.read_state_vector())
    } finally { runner.destroy(); runner.free() }
  }, SELECT_BELL)
  expect(values).toEqual(bellStates[1])
})

test('low-level selection rejects numeric strings without coercion', async ({ page }) => {
  await hostEmbed(page, SELECT_BELL)
  const error = await page.evaluate(async circuit => {
    const w = window as any
    w.runner.destroy()
    const runner = await w.embedWasm.start_embed(w.canvas, circuit, true, undefined, undefined, false)
    try { runner.selectStep('1'); return null }
    catch (error) { return String(error) }
    finally { runner.destroy(); runner.free() }
  }, SELECT_BELL)
  expect(error).toContain('TypeError: selectStep index must be a number')
})

test('UI step click and selectStep lock the same state', async ({ page }) => {
  await hostEmbed(page, CNOT, { settings: { palette: ['H', 'X'] } })
  const selectedValues = await page.evaluate(async () => {
    const runner = (window as any).runner
    runner.selectStep(1)
    return Array.from(await runner.readStateVector())
  })
  await page.evaluate(() => (window as any).runner.selectStep(2))
  const box = (await page.locator('qni-webgpu-test canvas').boundingBox())!
  // One-row palette: q1 is at y=272; y=295 is outside gate bodies
  // but inside the step row. Column 1 is at x=162+56.
  await page.mouse.click(box.x + 218, box.y + 295)
  await page.mouse.move(box.x + 800, box.y + 100)
  await expect.poll(async () => page.evaluate(async () => Array.from(await (window as any).runner.readStateVector())))
    .toEqual(selectedValues)
})

test('runner readback follows UI hover', async ({ page }) => {
  await hostEmbed(page, CNOT, { settings: { palette: ['H', 'X'] } })
  await page.evaluate(() => (window as any).runner.selectStep(1))
  const box = (await page.locator('qni-webgpu-test canvas').boundingBox())!
  await page.mouse.move(box.x + 162, box.y + 295)
  await expect.poll(async () => page.evaluate(async () => Array.from(await (window as any).runner.readStateVector())))
    .toEqual([1, 0, 0, 0, 0, 0, 0, 0])
})

test('stepCount excludes block markers', async ({ page }) => {
  await hostEmbed(page, SELECT_BELL)
  expect(await page.evaluate(() => (window as any).runner.stepCount())).toBe(2)
})

test('selectedStep reports the locked step', async ({ page }) => {
  await hostEmbed(page, SELECT_BELL)
  expect(await page.evaluate(() => {
    const runner = (window as any).runner
    runner.selectStep(1)
    return runner.selectedStep()
  })).toBe(1)
})

for (const expression of ['-2', '2', '0.5', 'NaN', 'Infinity', '"1"', 'null', 'undefined', 'true', '1n']) {
  test(`selectStep rejects invalid index ${expression}`, async ({ page }) => {
    await hostEmbed(page, SELECT_BELL)
    const result = await page.evaluate(source => {
      const runner = (window as any).runner
      try { runner.selectStep(Function(`return (${source})`)()); return null }
      catch (error) { return String(error) }
    }, expression)
    const errorType = ['"1"', 'null', 'undefined', 'true', '1n'].includes(expression) ? 'TypeError' : 'RangeError'
    expect(result).toContain(`${errorType}: selectStep index must be`)
  })
}

test('invalid selectStep leaves the locked step unchanged', async ({ page }) => {
  await hostEmbed(page, SELECT_BELL)
  const selected = await page.evaluate(() => {
    const runner = (window as any).runner
    runner.selectStep(1)
    try { runner.selectStep(2) } catch { /* expected invalid index */ }
    return runner.selectedStep()
  })
  expect(selected).toBe(1)
})

test('empty circuit has no selected step', async ({ page }) => {
  await hostEmbed(page, '{"cols":[]}')
  expect(await page.evaluate(() => (window as any).runner.selectedStep())).toBeNull()
})

test('empty circuit rejects step zero', async ({ page }) => {
  await hostEmbed(page, '{"cols":[]}')
  const error = await page.evaluate(() => {
    try { (window as any).runner.selectStep(0); return null }
    catch (error) { return String(error) }
  })
  expect(error).toContain('integer in [-1, 0)')
})

test('destroyed runner rejects selection clearly', async ({ page }) => {
  await hostEmbed(page, SELECT_BELL)
  const error = await page.evaluate(() => {
    const runner = (window as any).runner
    runner.destroy()
    try { runner.selectStep(0); return null }
    catch (error) { return String(error) }
  })
  expect(error).toContain('Circuit runner is destroyed')
})

test('runner reads a committed gate before its deferred paint frame', async ({ page }) => {
  await page.addInitScript(() => {
    const raf = window.requestAnimationFrame.bind(window)
    window.requestAnimationFrame = callback => raf(time => {
      if (!(window as any).freezeFrames) callback(time)
    })
  })
  await hostEmbed(page, '{"cols":[]}', { settings: { palette: ['X'] } })
  await waitForStepZero(page)
  const box = (await page.locator('qni-webgpu-test canvas').boundingBox())!
  await page.mouse.move(box.x + 480, box.y + 100)
  await page.mouse.down()
  await page.mouse.move(box.x + 300, box.y + 180, { steps: 8 })
  await page.mouse.move(box.x + 162, box.y + 216, { steps: 8 })
  await page.evaluate(() => { (window as any).freezeFrames = true })
  await page.mouse.up()
  const values = await page.evaluate(async () => {
    const runner = (window as any).runner
    runner.selectStep(0)
    return Array.from(await runner.readStateVector())
  })
  expect(values).toEqual([0, 0, 1, 0])
})

test('runner keeps rejecting readback after snapshot capacity failure and selection', async ({ page }) => {
  const cols = Array.from({ length: 258 }, () => ['X'])
  await hostEmbed(page, JSON.stringify({ cols }), { settings: { showStatePanel: false } })
  const error = await page.evaluate(async () => {
    const runner = (window as any).runner
    try { await runner.readStateVector() } catch { /* selection must not clear this error */ }
    runner.selectStep(0)
    try { await runner.readStateVector(); return null } catch (error) { return String(error) }
  })
  expect(error).toContain('MAX_STEP_SNAPSHOT_SLOTS')
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

test('narrow single-wire embed selects the initial gap', async ({ page }) => {
  await hostEmbed(page, '{"cols":[["H"]]}', { ...H_GATE_TUTORIAL, width: 390, height: 560 })
  await waitForStepZero(page)
  const box = (await page.locator('qni-webgpu-test canvas').boundingBox())!
  // Compact slot 0 is x=88, and its left edge is x=60.
  await page.mouse.click(box.x + 54, box.y + 216)
  await page.mouse.move(box.x + 380, box.y + 400)
  await expect.poll(() => page.evaluate(() => (window as any).runner.selectedStep())).toBe(-1)
})
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

// eframe's TextAgent INPUT must not take the page's autofocus: the browser
// would scroll a tutorial article to the embed on every load.
const textAgent = (page: Page) => page.locator('qni-webgpu-test input')

test('embed text agent input has no autofocus', async ({ page }) => {
  await hostEmbed(page)
  expect(await textAgent(page).evaluate(input => (input as HTMLInputElement).autofocus)).toBe(false)
})

test('embed below a long article leaves the page at the top', async ({ page }) => {
  await hostEmbed(page, '{"cols":[["H"]]}', { articleHeight: 3000, fragment: '' })
  await waitForHState(page)
  // Autofocus runs at a rendering update; give the browser a few of them.
  const scrollY = await page.evaluate(async () => {
    for (let i = 0; i < 10; i++) await new Promise(requestAnimationFrame)
    return window.scrollY
  })
  expect(scrollY).toBe(0)
})

// The angle editor is egui text input, which eframe routes through the
// TextAgent INPUT. In the 960x640 host with an empty palette the π/2 label
// sits above the gate at (170, 107).
test('embed angle editor accepts typed text through the text agent', async ({ page }) => {
  await hostEmbed(page, '{"cols":[["P(π_2)"]]}', { settings: { palette: [] } })
  await waitForStepZero(page)
  await page.locator('qni-webgpu-test canvas').click({ position: { x: 170, y: 107 } })
  await page.waitForFunction(() => {
    const root = document.querySelector('qni-webgpu-test')!.shadowRoot!
    return root.activeElement === root.querySelector('input')
  }, null, { timeout: 5000 })
  await page.keyboard.type('1/4')
  await page.keyboard.press('Enter')
  await expect.poll(() => circuitJSON(page)).toBe('{"cols":[["P(π_4)"]]}')
})

for (const showStatePanel of [true, false]) {
  test(`selectStep -1 immediately reads initial state (panel=${showStatePanel})`, async ({ page }) => {
    await hostEmbed(page, SELECT_BELL, { settings: { showStatePanel } })
    const values = await page.evaluate(async () => {
      const runner = (window as any).runner
      runner.selectStep(-1)
      await runner.readStateVector()
      runner.selectStep(1)
      await runner.readStateVector()
      runner.selectStep(-1)
      return Array.from(await runner.readStateVector())
    })
    expect(values).toEqual([1, 0, 0, 0, 0, 0, 0, 0])
  })
}

test('selectedStep reports initial state as a numeric minus one', async ({ page }) => {
  await hostEmbed(page, SELECT_BELL)
  const selected = await page.evaluate(() => {
    const runner = (window as any).runner
    runner.selectStep(-1)
    return runner.selectedStep()
  })
  expect(selected).toBe(-1)
})
