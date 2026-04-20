const test = require('node:test')
const assert = require('node:assert/strict')

const shared = require('../test-support/browser-launch.cjs')
const browserPolicy = require('../playwright-browser.cjs')

test('playwright-browser remains a thin wrapper over the shared browser policy module', () => {
  assert.equal(browserPolicy.resolvePlaywrightBrowserExecutable, shared.resolvePlaywrightBrowserExecutable)
  assert.equal(browserPolicy.getStandardWebGpuLaunchOptions, shared.getStandardWebGpuLaunchOptions)
  assert.equal(browserPolicy.getPlainChromiumLaunchOptions, shared.getPlainChromiumLaunchOptions)
})
