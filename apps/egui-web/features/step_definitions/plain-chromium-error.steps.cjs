const assert = require('node:assert/strict')
const { Given, When, Then } = require('@cucumber/cucumber')

const { PLAIN_BROWSER_MODE, openPageForMode } = require('../support/browser.cjs')
const { openEguiApp, waitForAppReady, readEguiError } = require('../support/egui-helpers.cjs')

const CUCUMBER_STEP_TIMEOUT_MS = 20_000

Given('the egui web app is open in plain chromium', { timeout: CUCUMBER_STEP_TIMEOUT_MS }, async function () {
  const page = await openPageForMode(this, PLAIN_BROWSER_MODE)
  await openEguiApp(page, this.baseUrl)
})

When('the plain chromium session finishes loading', { timeout: CUCUMBER_STEP_TIMEOUT_MS }, async function () {
  assert.ok(this.page, 'expected egui page to be open')
  await waitForAppReady(this.page)
})

Then('a visible WebGPU error is shown', async function () {
  assert.ok(this.page, 'expected egui page to be open')

  await this.page.waitForFunction(() => Boolean(window.__eguiError), null, {
    timeout: 20_000,
  })

  assert.notEqual(await readEguiError(this.page), null)

  const errorLocator = this.page.locator('[data-testid="webgpu-error"]')
  await errorLocator.waitFor({ state: 'visible' })
  assert.match(await errorLocator.innerText(), /WebGPU/i)
})
