import { test, expect } from '@playwright/test'
import { execFileSync } from 'node:child_process'
import { EmbedContractHost as Host } from '../test-support/embed-api-contract'

const H = '{"cols":[["H"]]}'
const SIGNED = '{"cols":[["H"],["Z"]]}'
const METHODS = ['circuitJSON', 'preferredHeight', 'selectStep', 'selectedStep', 'stepCount', 'readStateVector'] as const

// Each worker owns a real second HTTP listener, never a fixed port or routed asset.
test.beforeAll(() => Host.listen())
test.afterAll(() => Host.close())
test.beforeEach(async ({ page }) => Host.open(page))

test('nested cross-origin distribution exports its API and build versions', async ({ page }) => {
  const sourceCommit = process.env.QNI_SOURCE_COMMIT || process.env.GITHUB_SHA || execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim()
  expect(await page.evaluate(() => {
    const api = (window as any).api
    return { version: api.EMBED_API_VERSION, sha: api.SOURCE_COMMIT }
  })).toEqual({ version: 1, sha: sourceCommit })
})

test('fixture serves published artifacts with JavaScript/Wasm MIME and CORS', async ({ request }) => {
  const responses = []
  for (const file of ['qni-embed.mjs', 'qni-web.js', 'qni-web_bg.wasm']) {
    const response = await request.get(`${Host.assetURL}${file}`)
    responses.push({ status: response.status(), mime: response.headers()['content-type'], cors: response.headers()['access-control-allow-origin'] })
    await response.dispose()
  }
  expect(responses).toEqual([
    { status: 200, mime: 'text/javascript', cors: '*' },
    { status: 200, mime: 'text/javascript', cors: '*' },
    { status: 200, mime: 'application/wasm', cors: '*' },
  ])
})

test('public error constructor and classifier preserve cause, detail and stable codes', async ({ page }) => {
  expect(await page.evaluate(() => {
    const api = (window as any).api
    const cause = new Error('underlying failure')
    const error = new api.QniStartupError('load-failed', 'download failed', { cause, detail: { status: 503 } })
    return {
      name: error.name, message: error.message, code: error.code, detail: error.detail,
      isError: error instanceof Error, causePreserved: error.cause === cause,
      identity: api.classifyStartupError(error) === error,
      networkCode: api.classifyStartupError(new TypeError('Failed to fetch')).code,
      unknownCode: new api.QniStartupError('not-a-code', 'unknown').code,
      codes: [...api.STARTUP_ERROR_CODES], frozen: Object.isFrozen(api.STARTUP_ERROR_CODES),
    }
  })).toEqual({
    name: 'QniStartupError', message: 'download failed', code: 'load-failed', detail: { status: 503 },
    isError: true, causePreserved: true, identity: true, networkCode: 'load-failed', unknownCode: 'unknown',
    codes: ['insecure-context', 'wasm-unavailable', 'webgpu-unsupported', 'gpu-adapter-unavailable',
      'gpu-device-unavailable', 'gpu-device-lost', 'load-failed', 'startup-timeout', 'invalid-embed', 'unknown'], frozen: true,
  })
})

test('public preflight and diagnosis accept this real WebGPU browser', async ({ page }) => {
  expect(await page.evaluate(async () => {
    const api = (window as any).api
    return { preflight: api.preflightSync(), diagnosis: await api.diagnoseWebGpu() }
  })).toEqual({ preflight: null, diagnosis: null })
})

test('public error renderer exposes platform-aware content and removable DOM', async ({ page }) => {
  expect(await page.evaluate(() => {
    const api = (window as any).api
    const platform = api.detectPlatform({ userAgent: 'Mozilla/5.0 (Windows NT 10.0) Chrome/140.0.0.0 Safari/537.36' })
    const error = new api.QniStartupError('load-failed', 'download failed')
    const content = api.startupErrorContent(error, platform)
    const container = document.createElement('div')
    document.body.append(container)
    const handle = api.renderStartupError(container, error, { platform })
    const heading = container.firstElementChild!.shadowRoot!.querySelector('h3')!.textContent
    const height = handle.naturalHeight()
    handle.remove()
    return { platform, code: handle.code, titleMatches: heading === content.title, heightValid: Number.isFinite(height) && height > 0, removed: container.childElementCount === 0 }
  })).toEqual({ platform: { os: 'windows', browser: 'chrome', version: 140, mobile: false }, code: 'load-failed', titleMatches: true, heightValid: true, removed: true })
})

test('invalid circuit startup rejects with the exported structured error type', async ({ page }) => {
  expect(await page.evaluate(async () => {
    const w = window as any
    try { await w.start('{"cols":[["unknown-gate"]]}'); return null }
    catch (error: any) { return { name: error.name, code: error.code, instance: error instanceof w.api.QniStartupError } }
  })).toEqual({ name: 'QniStartupError', code: 'invalid-embed', instance: true })
})

test('circuitJSON is synchronous and preserves the canonical circuit', async ({ page }) => {
  await Host.start(page, SIGNED)
  expect(await page.evaluate(() => {
    const json = (window as any).runners[0].circuitJSON()
    return { type: typeof json, json, thenable: !!json?.then }
  })).toEqual({ type: 'string', json: SIGNED, thenable: false })
})

test('startup progress includes download, compile, GPU and prepare with byte counts', async ({ page }) => {
  await Host.start(page)
  expect(await page.evaluate(() => {
    const updates = (window as any).progress
    const stages = [...new Set(updates.map((update: any) => update.stage))]
    return {
      stages,
      bytes: updates.some((update: any) => update.stage === 'download' && update.loaded > 0),
      valid: updates.every((update: any) => Number.isFinite(update.loaded) && update.loaded >= 0 &&
        (update.total === null || (Number.isFinite(update.total) && update.total >= update.loaded))),
      final: updates.at(-1).stage,
    }
  })).toEqual({ stages: ['download', 'compile', 'gpu', 'prepare'], bytes: true, valid: true, final: 'prepare' })
})

for (const palette of [undefined, [], ['H', 'X']]) {
  for (const showStatePanel of [false, true]) {
    test(`getters and immediate signed readback with palette=${JSON.stringify(palette)} panel=${showStatePanel}`, async ({ page }) => {
      await Host.start(page, SIGNED, { palette, showStatePanel, maxWireCount: 1 })
      expect(await page.evaluate(async () => {
        const runner = (window as any).runners[0]
        const initial = runner.selectedStep()
        runner.selectStep(1)
        const height = runner.preferredHeight(960)
        return {
          initial, selected: runner.selectedStep(), count: runner.stepCount(),
          heightValid: Number.isFinite(height) && height > 0,
          values: Array.from(await runner.readStateVector(), (value: number) => Math.round(value * 1e5) / 1e5 || 0),
        }
      })).toEqual({ initial: 0, selected: 1, count: 2, heightValid: true, values: [0.70711, 0, -0.70711, 0] })
    })
  }
}

test('palette, state panel and wire-limit settings affect the preferred layout', async ({ page }) => {
  const settings = [
    { showStatePanel: false, maxWireCount: 1 },
    { showStatePanel: false, maxWireCount: 1, palette: [] },
    { showStatePanel: false, maxWireCount: 1, palette: ['H', 'X'] },
    { showStatePanel: true, maxWireCount: 1, palette: [] },
    { showStatePanel: false, palette: [] },
  ]
  const heights: number[] = []
  for (const setting of settings) {
    const index = await Host.start(page, H, setting)
    heights.push(await page.evaluate(index => {
      const runner = (window as any).runners[index]
      const height = runner.preferredHeight(960)
      runner.destroy()
      return height
    }, index))
  }
  expect({
    fullPalette: heights[0] > heights[2],
    restrictedPalette: heights[2] > heights[1],
    statePanel: heights[3] > heights[1],
    wireLimit: heights[4] > heights[1],
  }).toEqual({ fullPalette: true, restrictedPalette: true, statePanel: true, wireLimit: true })
})

test('eleven distinct four-qubit circuits remain isolated through immediate selection/readback', async ({ page }) => {
  expect(await page.evaluate(async () => {
    const w = window as any
    const circuits = Array.from({ length: 11 }, (_, index) => JSON.stringify({
      cols: [Array(4).fill('|0>'), Array.from({ length: 4 }, (_, bit) => (index + 1) & (1 << bit) ? 'X' : 1)],
    }))
    await Promise.all(circuits.map(circuit => w.start(circuit, { palette: [], showStatePanel: false })))
    // Change every selection before issuing concurrent reads in a different order.
    const order = [10, 0, 8, 1, 6, 2, 4, 3, 5, 7, 9]
    w.runners.forEach((runner: any) => runner.selectStep(1))
    const finals = await Promise.all(order.map(async index => ({
      identity: index, final: Array.from(await w.runners[index].readStateVector()),
    })))
    w.runners.forEach((runner: any) => runner.selectStep(-1))
    const results = await Promise.all(finals.map(async result => ({
      ...result, initial: Array.from(await w.runners[result.identity].readStateVector()),
    })))
    return results.sort((a, b) => a.identity - b.identity)
  })).toEqual(Array.from({ length: 11 }, (_, index) => ({
    identity: index,
    final: Array.from({ length: 32 }, (_, component) => component === (index + 1) * 2 ? 1 : 0),
    initial: [1, ...Array(31).fill(0)],
  })))
})

test('disposing one runner leaves every remaining runner usable', async ({ page }) => {
  await Host.start(page, H, { palette: [] })
  await Host.start(page, '{"cols":[["X"]]}', { palette: [] })
  await Host.start(page, SIGNED, { palette: [] })
  expect(await page.evaluate(async () => {
    const w = window as any
    w.runners[1].destroy()
    w.runners[1].destroy()
    return Promise.all([w.runners[0], w.runners[2]].map(async runner => {
      runner.selectStep(runner.stepCount() - 1)
      return { json: runner.circuitJSON(), values: Array.from(await runner.readStateVector(), (value: number) => Math.round(value * 1e5) / 1e5 || 0) }
    }))
  })).toEqual([
    { json: H, values: [0.70711, 0, 0.70711, 0] },
    { json: SIGNED, values: [0.70711, 0, -0.70711, 0] },
  ])
})

for (const method of METHODS) {
  test(`${method} rejects after destroy with the stable Error contract`, async ({ page }) => {
    await Host.start(page)
    const error = await page.evaluate(async method => {
      const runner = (window as any).runners[0]
      runner.destroy()
      try { await runner[method](method === 'preferredHeight' ? 960 : 0); return null }
      catch (error: any) { return { name: error.name, message: error.message, code: error.code } }
    }, method)
    expect(error).toEqual({ name: 'Error', message: 'Circuit runner is destroyed', code: 'runner-destroyed' })
  })
}

test('destroying the real shared GPU device allows every circuit to export and recover on a new device', async ({ page }) => {
  await page.evaluate(() => {
    const w = window as any
    w.devices = []
    const requestDevice = GPUAdapter.prototype.requestDevice
    GPUAdapter.prototype.requestDevice = async function (descriptor?: GPUDeviceDescriptor) {
      const device = await requestDevice.call(this, descriptor)
      w.devices.push(device)
      return device
    }
  })
  await Host.start(page, H, { palette: [] })
  await Host.start(page, SIGNED, { palette: [] })
  await page.evaluate(() => (window as any).devices[0].destroy())
  await page.waitForFunction(() => (window as any).losses.length === 2)
  expect(await page.evaluate(async () => {
    const w = window as any
    const circuits = w.runners.map((runner: any) => runner.circuitJSON())
    w.runners.forEach((runner: any) => runner.destroy())
    const recovered = await Promise.all(circuits.map((circuit: string, index: number) =>
      w.api.startEmbed(w.canvases[index], circuit, { palette: [] })))
    const states = await Promise.all(recovered.map(async (runner: any) => {
      runner.selectStep(runner.stepCount() - 1)
      return Array.from(await runner.readStateVector(), (value: number) => Math.round(value * 1e5) / 1e5 || 0)
    }))
    recovered.forEach((runner: any) => runner.destroy())
    return { circuits, states, devices: w.devices.length, codes: w.losses.map((error: any) => error.code) }
  })).toEqual({ circuits: [H, SIGNED], states: [[0.70711, 0, 0.70711, 0], [0.70711, 0, -0.70711, 0]], devices: 2, codes: ['gpu-device-lost', 'gpu-device-lost'] })
})

test('simulated shared device loss notifies all live callbacks and keeps JSON export usable', async ({ page }) => {
  await Host.start(page, H)
  await Host.start(page, SIGNED)
  expect(await page.evaluate(() => {
    const w = window as any
    dispatchEvent(new CustomEvent('qni-device-lost', { detail: { reason: 'unknown', message: 'contract simulation' } }))
    return { losses: w.losses, json: w.runners.map((runner: any) => runner.circuitJSON()) }
  })).toEqual({ losses: [{ index: 0, code: 'gpu-device-lost' }, { index: 1, code: 'gpu-device-lost' }], json: [H, SIGNED] })
})

for (const method of METHODS.filter(method => method !== 'circuitJSON')) {
  test(`${method} rejects after simulated device loss with gpu-device-lost`, async ({ page }) => {
    await Host.start(page)
    const error = await page.evaluate(async method => {
      const runner = (window as any).runners[0]
      dispatchEvent(new CustomEvent('qni-device-lost'))
      try { await runner[method](method === 'preferredHeight' ? 960 : 0); return null }
      catch (error: any) { return error.code }
    }, method)
    expect(error).toBe('gpu-device-lost')
  })
}

test('destroy after simulated device loss detaches callbacks and permits restart on the same canvas', async ({ page }) => {
  await Host.start(page)
  expect(await page.evaluate(async () => {
    const w = window as any
    dispatchEvent(new CustomEvent('qni-device-lost'))
    const json = w.runners[0].circuitJSON()
    w.runners[0].destroy()
    w.runners[0].destroy()
    const runner = await w.api.startEmbed(w.canvases[0], json, { palette: [] })
    runner.selectStep(0)
    const values = Array.from(await runner.readStateVector(), (value: number) => Math.round(value * 1e5) / 1e5 || 0)
    runner.destroy()
    dispatchEvent(new CustomEvent('qni-device-lost'))
    return { values, losses: w.losses }
  })).toEqual({ values: [0.70711, 0, 0.70711, 0], losses: [{ index: 0, code: 'gpu-device-lost' }] })
})

for (const action of ['pointer', 'Enter', 'Space'] as const) {
  test(`onRestart is reachable by ${action}${action === 'pointer' ? '' : ' after reverse Tab'}`, async ({ page }) => {
    await Host.start(page, SIGNED, { palette: [], showRestartButton: true })
    await Host.frames(page)
    const canvas = Host.canvas(page)
    if (action === 'pointer') {
      // Same default 560px state-panel geometry as embed.spec.ts.
      await canvas.click({ position: { x: 480 + 280 - 32, y: 640 - 64 - 8 - 160 - 16 } })
    } else {
      await canvas.focus()
      await page.keyboard.press('Shift+Tab')
      await Host.frames(page)
      await page.keyboard.press(action)
    }
    await expect.poll(() => page.evaluate(() => (window as any).restarts))
      .toEqual([{ index: 0, circuit: SIGNED }])
  })
}

for (const action of ['edit', 'undo', 'redo'] as const) {
  test(`${action} resets a selected column to the last step and returns the signed vector`, async ({ page }) => {
    const edited = '{"cols":[["H"],["Z"],["X"]]}'
    await Host.start(page, SIGNED, { palette: ['H', 'X'], maxWireCount: 1 })
    await Host.frames(page)
    await page.evaluate(() => (window as any).runners[0].selectStep(-1))
    await Host.drop(page, 504, 274)
    await Host.waitForCircuit(page, edited)
    if (action !== 'edit') {
      await page.evaluate(() => (window as any).runners[0].selectStep(-1))
      await Host.canvas(page).click({ position: { x: 28, y: 22 } })
      await Host.waitForCircuit(page, SIGNED)
    }
    if (action === 'redo') {
      await page.evaluate(() => (window as any).runners[0].selectStep(-1))
      await Host.canvas(page).click({ position: { x: 68, y: 22 } })
      await Host.waitForCircuit(page, edited)
    }
    await page.mouse.move(900, 100)
    expect(await page.evaluate(async () => {
      const runner = (window as any).runners[0]
      return { selected: runner.selectedStep(), json: runner.circuitJSON(), values: Array.from(await runner.readStateVector(), (value: number) => Math.round(value * 1e5) / 1e5 || 0) }
    })).toEqual({
      selected: action === 'undo' ? 1 : 2,
      json: action === 'undo' ? SIGNED : edited,
      values: action === 'undo' ? [0.70711, 0, -0.70711, 0] : [-0.70711, 0, 0.70711, 0],
    })
  })
}

test('maxWireCount prevents adding a second wire but permits a first-wire edit', async ({ page }) => {
  await Host.start(page, '{"cols":[["|0>"]]}', { palette: ['H', 'X'], maxWireCount: 1 })
  await Host.frames(page)
  await Host.drop(page, 456, 218, 272)
  await Host.drop(page, 504, 218)
  await expect.poll(() => page.evaluate(() => (window as any).runners[0].circuitJSON()))
    .toBe('{"cols":[["|0>"],["X"]]}')
})

test('host destroy/start reruns sample both correlated measurement outcomes', async ({ page }) => {
  await Host.start(page, '{"cols":[["H"],["Measure>a"],[1,"X<a"]]}', { palette: [], showStatePanel: false })
  expect(await page.evaluate(async () => {
    const w = window as any
    const canvas = w.canvases[0]
    let runner = w.runners[0]
    const outcomes = new Set<string>()
    for (let attempt = 0; attempt < 40; attempt++) {
      runner.selectStep(2)
      outcomes.add(JSON.stringify(Array.from(await runner.readStateVector(), (value: number) => Math.round(value * 1e5) / 1e5 || 0)))
      const circuit = runner.circuitJSON()
      runner.destroy()
      if (outcomes.size === 2 || attempt === 39) break
      runner = await w.api.startEmbed(canvas, circuit, { palette: [], showStatePanel: false })
    }
    return [...outcomes].sort()
  })).toEqual([
    JSON.stringify([0, 0, 0, 0, 0, 0, 1, 0]),
    JSON.stringify([1, 0, 0, 0, 0, 0, 0, 0]),
  ].sort())
})

test('a live runner keeps its measurement through selection, rendering and readback', async ({ page }) => {
  await Host.start(page, '{"cols":[["H"],["Measure>a"],[1,"X<a"]]}', { palette: [], showStatePanel: false })
  expect(await page.evaluate(async () => {
    const runner = (window as any).runners[0]
    runner.selectStep(2)
    const first = JSON.stringify(Array.from(await runner.readStateVector()))
    const results = []
    for (let attempt = 0; attempt < 5; attempt++) {
      runner.selectStep(-1)
      await runner.readStateVector()
      await new Promise(requestAnimationFrame)
      runner.selectStep(2)
      await new Promise(requestAnimationFrame)
      results.push(JSON.stringify(Array.from(await runner.readStateVector())) === first)
    }
    return results
  })).toEqual(Array(5).fill(true))
})

test('deterministic measurement variables and conditional gates survive step reselection', async ({ page }) => {
  await Host.start(page, '{"cols":[["X"],["Measure>a"],[1,"X<a"]]}', { palette: [], showStatePanel: false })
  expect(await page.evaluate(async () => {
    const runner = (window as any).runners[0]
    const results = []
    for (let i = 0; i < 3; i++) {
      runner.selectStep(2)
      results.push(Array.from(await runner.readStateVector()))
      runner.selectStep(-1)
      results.push(Array.from(await runner.readStateVector()))
    }
    return results
  })).toEqual(Array.from({ length: 6 }, (_, index) => index % 2 === 0 ? [0, 0, 0, 0, 0, 0, 1, 0] : [1, 0, 0, 0, 0, 0, 0, 0]))
})

test('removing the element during startup rejects cleanly without unhandled errors', async ({ page }) => {
  const errors: string[] = []
  page.on('pageerror', error => errors.push(error.message))
  await page.evaluate(() => {
    const w = window as any
    let release: () => void
    const gate = new Promise<void>(resolve => { release = resolve })
    const requestDevice = GPUAdapter.prototype.requestDevice
    GPUAdapter.prototype.requestDevice = async function (descriptor?: GPUDeviceDescriptor) {
      w.waitingForDevice = true
      await gate
      return requestDevice.call(this, descriptor)
    }
    w.releaseDevice = () => release()
    w.pendingStart = w.start('{"cols":[["H"]]}').then(() => 'fulfilled', (error: any) => error.code)
  })
  await page.waitForFunction(() => (window as any).waitingForDevice)
  const outcome = await page.evaluate(async () => {
    const w = window as any
    document.querySelector('qni-contract-test')!.remove()
    w.releaseDevice()
    const outcome = await w.pendingStart
    for (let i = 0; i < 4; i++) await new Promise(requestAnimationFrame)
    return { outcome, unhandled: w.unhandled }
  })
  expect({ ...outcome, errors }).toEqual({ outcome: 'invalid-embed', unhandled: [], errors: [] })
})
