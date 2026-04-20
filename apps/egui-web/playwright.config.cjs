const { defineConfig } = require('@playwright/test')
const { chromium } = require('playwright')
const { getStandardWebGpuLaunchOptions } = require('./test-support/browser-launch.cjs')
const { getWebServerConfig } = require('./test-support/web-server.cjs')

const webServer = getWebServerConfig()
const standardBrowser = getStandardWebGpuLaunchOptions({
  env: process.env,
  defaultPath: chromium.executablePath(),
})

module.exports = defineConfig({
  testDir: './tests',
  use: {
    baseURL: webServer.url,
    viewport: { width: 1000, height: 800 },
    browserName: 'chromium',
    headless: standardBrowser.headless,
    launchOptions: {
      executablePath: standardBrowser.executablePath,
      args: standardBrowser.args,
    },
  },
  webServer,
})
