import assert = require('node:assert/strict')
import { Given, Then, When } from '@cucumber/cucumber'
import type { Page } from 'playwright'
import type { EguiWorld } from '../support/world-types'

type BrowserSupport = {
  PLAIN_BROWSER_MODE: string
  openPageForMode: (world: EguiWorld, mode: string) => Promise<Page>
}

type EguiHelpers = {
  openEguiApp: (page: Page, baseUrl: string) => Promise<void>
  waitForAppReady: (page: Page) => Promise<void>
  readEguiError: (page: Page) => Promise<string | null>
}

type WindowWithEguiError = Window & {
  __eguiError?: unknown
}

const { PLAIN_BROWSER_MODE, openPageForMode } = require('../support/browser.cjs') as BrowserSupport
const { openEguiApp, waitForAppReady, readEguiError } = require('../support/egui-helpers.cjs') as EguiHelpers

const CUCUMBER_STEP_TIMEOUT_MS = 20_000

const requirePage = (world: EguiWorld): Page => {
  assert.ok(world.page, 'expected egui page to be open')
  return world.page
}

Given(
  'the egui web app is open in plain chromium',
  { timeout: CUCUMBER_STEP_TIMEOUT_MS },
  async function (this: EguiWorld) {
    const page = await openPageForMode(this, PLAIN_BROWSER_MODE)
    await openEguiApp(page, this.baseUrl)
  }
)

When('the plain chromium session finishes loading', { timeout: CUCUMBER_STEP_TIMEOUT_MS }, async function (this: EguiWorld) {
  await waitForAppReady(requirePage(this))
})

Then('a visible WebGPU error is shown', async function (this: EguiWorld) {
  const page = requirePage(this)

  await page.waitForFunction(() => Boolean((window as WindowWithEguiError).__eguiError), null, {
    timeout: 20_000,
  })

  assert.notEqual(await readEguiError(page), null)

  const errorLocator = page.locator('[data-testid="webgpu-error"]')
  await errorLocator.waitFor({ state: 'visible' })
  assert.match(await errorLocator.innerText(), /WebGPU/i)
})
