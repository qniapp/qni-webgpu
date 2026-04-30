const test = require('node:test')
const assert = require('node:assert/strict')

const shared = require('../test-support/browser-launch.cjs')
const browserPolicy = require('../playwright-browser.cjs')

test('playwright-browser remains a thin wrapper over the shared browser policy module', () => {
  assert.equal(browserPolicy.resolvePlaywrightBrowserExecutable, shared.resolvePlaywrightBrowserExecutable)
  assert.equal(browserPolicy.getStandardWebGpuLaunchOptions, shared.getStandardWebGpuLaunchOptions)
  assert.equal(browserPolicy.getPlainChromiumLaunchOptions, shared.getPlainChromiumLaunchOptions)
})

test('plain chromium launch options can reuse a system-installed browser when available', () => {
  const launchOptions = shared.getPlainChromiumLaunchOptions({
    env: {},
    defaultPath: '/playwright/chromium',
    commandLookup: (name) => (name === 'google-chrome-stable' ? '/usr/bin/google-chrome-stable' : null),
  })

  assert.equal(launchOptions.executablePath, '/usr/bin/google-chrome-stable')
})
