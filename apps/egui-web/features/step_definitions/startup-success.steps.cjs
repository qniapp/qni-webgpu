const assert = require('node:assert/strict')
const { Given, When, Then } = require('@cucumber/cucumber')

const { STANDARD_BROWSER_MODE, openPageForMode } = require('../support/browser.cjs')
const {
  openEguiApp,
  waitForStartupReady,
  readEguiError,
  readStateVector,
  waitForCanvasContent,
  waitForStateVectorReady,
} = require('../support/egui-helpers.cjs')

const CUCUMBER_STEP_TIMEOUT_MS = 20_000

Given('the egui web app is open in the standard WebGPU browser', { timeout: CUCUMBER_STEP_TIMEOUT_MS }, async function () {
  const page = await openPageForMode(this, STANDARD_BROWSER_MODE)
  await openEguiApp(page, this.baseUrl)
})

When('the app finishes initializing', { timeout: CUCUMBER_STEP_TIMEOUT_MS }, async function () {
  assert.ok(this.page, 'expected egui page to be open')
  await waitForStartupReady(this.page, { waitForStateVector: true })
})

Then('the WebGPU error is absent', async function () {
  assert.ok(this.page, 'expected egui page to be open')
  assert.equal(await readEguiError(this.page), null)
})

Then('the canvas is visible', async function () {
  assert.ok(this.page, 'expected egui page to be open')
  await this.page.locator('#egui-canvas').waitFor({ state: 'visible' })
})

Then('the canvas renders non-background content', async function () {
  assert.ok(this.page, 'expected egui page to be open')
  const canvas = this.page.locator('#egui-canvas')
  await canvas.waitFor({ state: 'visible' })
  await waitForCanvasContent(this.page, canvas)
})

Then('the initial state vector is {string}', async function (expectedJson) {
  assert.ok(this.page, 'expected egui page to be open')
  await waitForStateVectorReady(this.page)
  assert.deepEqual(await readStateVector(this.page), JSON.parse(expectedJson))
})
