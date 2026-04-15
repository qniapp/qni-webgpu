const test = require('node:test')
const assert = require('node:assert/strict')

const {
  resolvePlaywrightBrowserExecutable,
} = require('../playwright-browser.cjs')

test('uses PLAYWRIGHT_CHROMIUM_PATH override first', () => {
  const actual = resolvePlaywrightBrowserExecutable({
    env: { PLAYWRIGHT_CHROMIUM_PATH: '/custom/browser' },
    defaultPath: '/bundled/chromium',
    commandLookup: () => null,
  })

  assert.equal(actual, '/custom/browser')
})

test('prefers google-chrome-stable when installed', () => {
  const actual = resolvePlaywrightBrowserExecutable({
    env: {},
    defaultPath: '/bundled/chromium',
    commandLookup: (name) => (name === 'google-chrome-stable' ? '/usr/bin/google-chrome-stable' : null),
  })

  assert.equal(actual, '/usr/bin/google-chrome-stable')
})

test('uses chrome binary when that is the available system browser', () => {
  const actual = resolvePlaywrightBrowserExecutable({
    env: {},
    defaultPath: '/bundled/chromium',
    commandLookup: (name) => (name === 'chrome' ? '/usr/bin/chrome' : null),
  })

  assert.equal(actual, '/usr/bin/chrome')
})

test('falls back to Playwright bundled chromium when no system browser is found', () => {
  const actual = resolvePlaywrightBrowserExecutable({
    env: {},
    defaultPath: '/bundled/chromium',
    commandLookup: () => null,
  })

  assert.equal(actual, '/bundled/chromium')
})
