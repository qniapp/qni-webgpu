const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

require('ts-node/register/transpile-only')
const shared = require('../test-support/browser-launch.ts')

test('playwright-browser compatibility wrapper is not shipped', async () => {
  await assert.rejects(
    () => fs.access(path.join(__dirname, '..', 'playwright-browser.cjs')),
    /ENOENT/
  )
})

test('plain chromium launch options can reuse a system-installed browser when available', () => {
  const launchOptions = shared.getPlainChromiumLaunchOptions({
    env: {},
    defaultPath: '/playwright/chromium',
    commandLookup: (name) => (name === 'google-chrome-stable' ? '/usr/bin/google-chrome-stable' : null),
  })

  assert.equal(launchOptions.executablePath, '/usr/bin/google-chrome-stable')
})
