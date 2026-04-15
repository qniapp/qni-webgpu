const test = require('node:test')
const assert = require('node:assert/strict')

const config = require('../playwright.config.cjs')

test('web server startup timeout allows cold CI trunk builds', () => {
  assert.ok(config.webServer, 'playwright config should define webServer')
  assert.equal(typeof config.webServer.timeout, 'number')
  assert.ok(
    config.webServer.timeout >= 180_000,
    `expected webServer.timeout >= 180000, got ${config.webServer.timeout}`
  )
})
