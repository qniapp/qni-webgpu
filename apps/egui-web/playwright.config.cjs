const { defineConfig } = require('@playwright/test')
const { chromium } = require('playwright')
const { resolvePlaywrightBrowserExecutable } = require('./playwright-browser.cjs')

module.exports = defineConfig({
  testDir: './tests',
  use: {
    baseURL: 'http://127.0.0.1:4174',
    viewport: { width: 1000, height: 800 },
    browserName: 'chromium',
    headless: process.env.HEADLESS !== '0',
    launchOptions: {
      executablePath: resolvePlaywrightBrowserExecutable({
        env: process.env,
        defaultPath: chromium.executablePath(),
      }),
      args: [
        '--enable-features=WebGPU,WebGPUDeveloperFeatures,WebGPUService,Vulkan',
        '--enable-unsafe-webgpu',
        '--enable-dawn-features=allow_unsafe_apis,enable_immediate_error_handling',
        '--ignore-gpu-blocklist',
        '--disable-gpu-sandbox',
        '--no-sandbox',
        '--use-gl=angle',
        '--use-angle=swiftshader',
        '--use-vulkan=swiftshader',
      ],
    },
  },
  webServer: {
    command: 'env -u NO_COLOR TRUNK_COLOR=never trunk serve --address 127.0.0.1 --port 4174 --no-autoreload',
    url: 'http://127.0.0.1:4174',
    timeout: 180_000,
    reuseExistingServer: true,
  },
})
