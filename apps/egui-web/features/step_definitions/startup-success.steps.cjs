const assert = require('node:assert/strict')
const { Given, When, Then } = require('@cucumber/cucumber')

const { STANDARD_BROWSER_MODE, openPageForMode } = require('../support/browser.cjs')
const {
  openEguiApp,
  waitForAppReady,
  readEguiError,
  readStateVector,
  waitForStateVectorReady,
} = require('../support/egui-helpers.cjs')

Given('the egui web app is open in the standard WebGPU browser', async function () {
  const page = await openPageForMode(this, STANDARD_BROWSER_MODE)
  await openEguiApp(page, this.baseUrl)
})

When('the app finishes initializing', async function () {
  assert.ok(this.page, 'expected egui page to be open')
  await waitForAppReady(this.page)
  await waitForStateVectorReady(this.page)
})

Then('the WebGPU error is absent', async function () {
  assert.ok(this.page, 'expected egui page to be open')
  assert.equal(await readEguiError(this.page), null)
})

Then('the canvas is visible', async function () {
  assert.ok(this.page, 'expected egui page to be open')
  await this.page.locator('#egui-canvas').waitFor({ state: 'visible' })
})

Then('the initial state vector is {string}', async function (expectedJson) {
  assert.ok(this.page, 'expected egui page to be open')
  await waitForStateVectorReady(this.page)
  assert.deepEqual(await readStateVector(this.page), JSON.parse(expectedJson))
})
