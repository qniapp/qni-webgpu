const { defineConfig } = require('@playwright/test')
const { chromium } = require('playwright')
const { getStandardWebGpuLaunchOptions } = require('./test-support/browser-launch.cjs')
const {
  getPlaywrightBaseUrl,
  getPlaywrightWebServerConfig,
} = require('./test-support/web-server.cjs')

const webServer = getPlaywrightWebServerConfig({ env: process.env })
const standardBrowser = getStandardWebGpuLaunchOptions({
  env: process.env,
  defaultPath: chromium.executablePath(),
})

module.exports = defineConfig({
  testDir: './tests',
  use: {
    baseURL: getPlaywrightBaseUrl({ env: process.env }),
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
