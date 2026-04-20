const test = require('node:test')
const assert = require('node:assert/strict')

const {
  evaluateWithRetry,
  waitForCanvasContent,
  waitForStartupReady,
  waitForStateVectorReady,
} = require('../features/support/egui-helpers.cjs')

const makePage = ({ evaluateImpl } = {}) => {
  const calls = {
    evaluate: [],
    waitForLoadState: [],
    waitForFunction: [],
  }

  return {
    calls,
    async evaluate(fn, arg) {
      calls.evaluate.push({ source: fn.toString(), arg })
      return evaluateImpl(fn, arg, calls.evaluate.length - 1)
    },
    async waitForLoadState(state) {
      calls.waitForLoadState.push(state)
    },
    async waitForFunction(fn, arg, options) {
      calls.waitForFunction.push({ source: fn.toString(), arg, options })
    },
  }
}

const makeLocator = ({ screenshotImpl } = {}) => {
  const calls = {
    screenshot: [],
  }

  return {
    calls,
    async screenshot(options) {
      calls.screenshot.push(options)
      return screenshotImpl ? screenshotImpl(options, calls.screenshot.length - 1) : Buffer.from('png')
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
