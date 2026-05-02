import { defineConfig } from '@playwright/test'
import { chromium } from 'playwright'
import { getStandardWebGpuLaunchOptions } from './test-support/browser-launch'
import {
  getPlaywrightBaseUrl,
  getPlaywrightWebServerConfig,
} from './test-support/web-server'

const webServer = getPlaywrightWebServerConfig({ env: process.env })
const standardBrowser = getStandardWebGpuLaunchOptions({
  env: process.env,
  defaultPath: chromium.executablePath(),
})
const workers = process.env.CI ? 6 : undefined

export default defineConfig({
  testDir: './tests',
  fullyParallel: true,
  workers,
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
