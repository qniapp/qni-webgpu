const test = require('node:test')
const assert = require('node:assert/strict')

const {
  evaluateWithRetry,
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
