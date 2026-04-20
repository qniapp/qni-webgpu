const test = require('node:test')
const assert = require('node:assert/strict')

const { chromium } = require('playwright')
const {
  getStandardWebGpuLaunchOptions,
} = require('../test-support/browser-launch.cjs')
const { getWebServerConfig } = require('../test-support/web-server.cjs')
const config = require('../playwright.config.cjs')

test('playwright config uses the shared browser and web server policies', () => {
  const expectedBrowser = getStandardWebGpuLaunchOptions({
    env: process.env,
    defaultPath: chromium.executablePath(),
  })

  const expectedWebServer = getWebServerConfig()

  assert.equal(config.use.baseURL, expectedWebServer.url)
  assert.equal(config.use.headless, expectedBrowser.headless)
  assert.deepEqual(config.use.launchOptions, {
    executablePath: expectedBrowser.executablePath,
    args: expectedBrowser.args,
  })
  assert.deepEqual(config.webServer, expectedWebServer)
})
