const test = require('node:test')
const assert = require('node:assert/strict')

const {
  dragPointer,
  evaluateWithRetry,
  getDragPreviewAboveStatePanelProbe,
  releasePointer,
  sampleCanvasPixels,
  waitForCanvasContent,
  waitForStartupReady,
  waitForStateVectorReady,
} = require('../features/support/egui-helpers.ts')

type MockEvaluateCall = {
  source: string
  arg: any
}

type MockPageCalls = {
  evaluate: MockEvaluateCall[]
  waitForLoadState: string[]
  waitForFunction: Array<{ source: string; arg: any; options: any }>
}

type MockPageOptions = {
  evaluateImpl?: (fn: Function, arg: any, index: number) => any | Promise<any>
}

type BoundingBox = {
  x?: number
  y?: number
  width: number
  height: number
}

type MockLocatorOptions = {
  screenshotImpl?: (options: any, index: number) => Buffer | Promise<Buffer>
  boundingBoxImpl?: (index: number) => BoundingBox | Promise<BoundingBox>
}

type MockCanvasPageOptions = {
  box?: { x: number; y: number; width: number; height: number }
}

const makePage = ({ evaluateImpl = async () => null }: MockPageOptions = {}) => {
  const calls = {
    evaluate: [],
    waitForLoadState: [],
    waitForFunction: [],
  } as MockPageCalls

  return {
    calls,
    async evaluate(fn: Function, arg?: any) {
      calls.evaluate.push({ source: fn.toString(), arg })
      return evaluateImpl(fn, arg, calls.evaluate.length - 1)
    },
    async waitForLoadState(state: string) {
      calls.waitForLoadState.push(state)
    },
    async waitForFunction(fn: Function, arg?: any, options?: any) {
      calls.waitForFunction.push({ source: fn.toString(), arg, options })
    },
  }
}

const makeLocator = ({ screenshotImpl, boundingBoxImpl }: MockLocatorOptions = {}) => {
  const calls = {
    screenshot: [],
    boundingBox: 0,
  } as { screenshot: any[]; boundingBox: number }

  return {
    calls,
    async screenshot(options?: any) {
      calls.screenshot.push(options)
      return screenshotImpl ? screenshotImpl(options, calls.screenshot.length - 1) : Buffer.from('png')
    },
    async boundingBox() {
      calls.boundingBox += 1
      return boundingBoxImpl ? boundingBoxImpl(calls.boundingBox - 1) : { x: 10, y: 20, width: 1000, height: 800 }
    },
  }
}

const makeCanvasPage = ({ box = { x: 10, y: 20, width: 1000, height: 800 } }: MockCanvasPageOptions = {}) => {
  const calls = {
    locator: [],
    move: [],
    down: 0,
    up: 0,
    waitForTimeout: [],
  } as {
    locator: string[]
    move: Array<{ x: number; y: number; options: any }>
    down: number
    up: number
    waitForTimeout: number[]
  }

  return {
    calls,
    locator(selector: string) {
      calls.locator.push(selector)
      return {
        async boundingBox() {
          return box
        },
      }
    },
    mouse: {
      async move(x: number, y: number, options?: any) {
        calls.move.push({ x, y, options })
      },
      async down() {
        calls.down += 1
      },
      async up() {
        calls.up += 1
      },
    },
    async waitForTimeout(ms: number) {
      calls.waitForTimeout.push(ms)
    },
  }
}

test('evaluateWithRetry retries execution-context-destroyed failures after waiting for app readiness', async () => {
  const page = makePage({
    evaluateImpl: async (_fn, _arg, index) => {
      if (index === 0) {
        throw new Error('Execution context was destroyed, most likely because of a navigation')
      }
      return 'ok'
    },
  })

  const result = await evaluateWithRetry(page, () => 'ok')

  assert.equal(result, 'ok')
  assert.equal(page.calls.evaluate.length, 2)
  assert.deepEqual(page.calls.waitForLoadState, ['load'])
  assert.equal(page.calls.waitForFunction.length, 1)
})

test('evaluateWithRetry does not retry unexpected errors', async () => {
  const page = makePage({
    evaluateImpl: async () => {
      throw new Error('Unexpected evaluate failure')
    },
  })

  await assert.rejects(() => evaluateWithRetry(page, () => 'nope'), /Unexpected evaluate failure/)
  assert.equal(page.calls.evaluate.length, 1)
  assert.deepEqual(page.calls.waitForLoadState, [])
  assert.equal(page.calls.waitForFunction.length, 0)
})

test('waitForStartupReady waits for app readiness and optional state vector readiness', async () => {
  const page = makePage({
    evaluateImpl: async (fn) => {
      const source = fn.toString()
      if (source.includes('__eguiError')) {
        return null
      }
      if (source.includes('__eguiReadStateVector')) {
        return [1, 0, 0, 0]
      }
      return null
    },
  })

  const stateVector = await waitForStartupReady(page, {
    timeout: 1_000,
    waitForStateVector: true,
  })

  assert.deepEqual(stateVector, [1, 0, 0, 0])
  assert.equal(page.calls.waitForFunction.length, 1)
  assert.deepEqual(page.calls.waitForFunction[0].options, { timeout: 1_000 })
  assert.equal(page.calls.evaluate.length, 3)
})

test('waitForStartupReady fails fast when egui reports an app error after startup', async () => {
  const page = makePage({
    evaluateImpl: async (fn) => {
      const source = fn.toString()
      if (source.includes('__eguiError')) {
        return 'WebGPU adapter unavailable'
      }
      if (source.includes('__eguiReadStateVector')) {
        return []
      }
      return null
    },
  })

  await assert.rejects(
    () => waitForStartupReady(page, { timeout: 1_000, waitForStateVector: true }),
    /WebGPU adapter unavailable/
  )
  assert.equal(page.calls.waitForFunction.length, 1)
  assert.equal(page.calls.evaluate.length, 1)
})

test('waitForStateVectorReady fails fast when egui reports an app error', async () => {
  const page = makePage({
    evaluateImpl: async (fn) => {
      const source = fn.toString()
      if (source.includes('__eguiError')) {
        return 'WebGPU adapter unavailable'
      }
      if (source.includes('__eguiReadStateVector')) {
        return []
      }
      return null
    },
  })

  await assert.rejects(
    () => waitForStateVectorReady(page, 1_000),
    /WebGPU adapter unavailable/
  )
  assert.equal(page.calls.evaluate.length, 1)
})

test('waitForCanvasContent fails fast when egui reports an app error', async () => {
  const page = makePage({
    evaluateImpl: async (fn) => {
      const source = fn.toString()
      if (source.includes('__eguiError')) {
        return 'WebGPU adapter unavailable'
      }
      if (source.includes('nonBackground')) {
        return { nonBackground: 0, sampledPixels: 25 }
      }
      return null
    },
  })
  const locator = makeLocator()

  await assert.rejects(
    () => waitForCanvasContent(page, locator, { timeout: 1_000, minNonBackground: 40 }),
    /WebGPU adapter unavailable/
  )
  assert.equal(locator.calls.screenshot.length, 0)
  assert.equal(page.calls.evaluate.length, 1)
})

test('waitForCanvasContent reports the last sampled non-background count on timeout', async () => {
  const page = makePage({
    evaluateImpl: async (fn) => {
      const source = fn.toString()
      if (source.includes('__eguiError')) {
        return null
      }
      if (source.includes('nonBackground')) {
        return { nonBackground: 7, sampledPixels: 25 }
      }
      return null
    },
  })
  const locator = makeLocator()

  await assert.rejects(
    () => waitForCanvasContent(page, locator, { timeout: 1, minNonBackground: 40 }),
    /Timed out waiting for egui canvas to render non-background content \(nonBackground=7, expected >= 40\)/
  )
  assert.equal(locator.calls.screenshot.length, 1)
  assert.equal(page.calls.evaluate.length, 2)
})

test('dragPointer moves relative to the egui canvas and can keep the pointer pressed', async () => {
  const page = makeCanvasPage()

  await dragPointer(page, { x: 12, y: 34 }, { x: 56, y: 78 }, 8, false)

  assert.deepEqual(page.calls.locator, ['#egui-canvas'])
  assert.deepEqual(page.calls.move, [
    { x: 22, y: 54, options: undefined },
    { x: 66, y: 98, options: { steps: 8 } },
  ])
  assert.equal(page.calls.down, 1)
  assert.equal(page.calls.up, 0)
  assert.deepEqual(page.calls.waitForTimeout, [16, 16])
})

test('releasePointer releases relative to the egui canvas', async () => {
  const page = makeCanvasPage()

  await releasePointer(page, { x: 90, y: 45 })

  assert.deepEqual(page.calls.locator, ['#egui-canvas'])
  assert.deepEqual(page.calls.move, [{ x: 100, y: 65, options: undefined }])
  assert.equal(page.calls.up, 1)
})

test('sampleCanvasPixels passes screenshot bytes and css size into page evaluation', async () => {
  const page = makePage({
    evaluateImpl: async (_fn, arg) => arg,
  })
  const locator = makeLocator({
    screenshotImpl: async () => Buffer.from('png'),
    boundingBoxImpl: async () => ({ width: 320, height: 180 }),
  })
  const samples = [{ name: 'probe', x: 12, y: 34 }]

  const result = await sampleCanvasPixels(page, locator, samples)

  assert.deepEqual(result.samples, samples)
  assert.equal(result.cssWidth, 320)
  assert.equal(result.cssHeight, 180)
  assert.match(result.base64, /^[A-Za-z0-9+/=]+$/)
  assert.deepEqual(locator.calls.screenshot, [{ type: 'png' }])
  assert.equal(locator.calls.boundingBox, 1)
})

test('sampleCanvasPixels waits for state-vector readiness before retrying a destroyed screenshot context', async () => {
  const page = makePage({
    evaluateImpl: async (fn, arg) => {
      const source = fn.toString()
      if (source.includes('__eguiError')) {
        return null
      }
      if (source.includes('__eguiReadStateVector')) {
        return [1, 0, 0, 0]
      }
      return arg
    },
  })
  const locator = makeLocator({
    screenshotImpl: async (_options, index) => {
      if (index === 0) {
        throw new Error('Execution context was destroyed, most likely because of a navigation')
      }
      return Buffer.from('png')
    },
    boundingBoxImpl: async () => ({ width: 320, height: 180 }),
  })

  const result = await sampleCanvasPixels(page, locator, [{ name: 'probe', x: 12, y: 34 }])

  assert.equal(result.cssWidth, 320)
  assert.deepEqual(page.calls.waitForLoadState, ['load'])
  assert.equal(page.calls.waitForFunction.length, 1)
  assert.match(page.calls.evaluate[0].source, /__eguiError/)
  assert.match(page.calls.evaluate[1].source, /__eguiReadStateVector/)
  assert.equal(locator.calls.screenshot.length, 2)
})

test('getDragPreviewAboveStatePanelProbe preserves the current drag target contract', () => {
  const probe = getDragPreviewAboveStatePanelProbe(1000, 800)

  assert.deepEqual(probe.source, { x: 140, y: 80 })
  assert.ok(probe.handleCenter.y > probe.source.y)
  assert.equal(probe.dragFillPoint.name, 'fill')
  assert.equal(probe.dragFillPoint.x, probe.handleCenter.x + 10)
  assert.equal(probe.dragFillPoint.y, probe.handleCenter.y + 10)
  assert.equal(probe.sourceFillPoint.name, 'sourceFill')
  assert.equal(probe.sourceFillPoint.x, probe.source.x + 10)
  assert.equal(probe.sourceFillPoint.y, probe.source.y + 10)
})

export {}
