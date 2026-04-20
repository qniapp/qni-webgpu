const { After, Before, Status } = require('@cucumber/cucumber')

require('./world.cjs')

const { closeWorldBrowser } = require('./browser.cjs')
const { readEguiError, getScenarioArtifactPath } = require('./egui-helpers.cjs')
const { ensureSharedWebServer, shutdownSharedWebServer } = require('./server.cjs')

const registerSupportCode = (register) => {
  try {
    register()
  } catch (error) {
    if (!String(error).includes("isn't running")) {
      throw error
    }
  }
}

registerSupportCode(() => {
  Before(async function ({ pickle }) {
    this.startScenario(pickle?.name)
    this.server = await ensureSharedWebServer()
    this.baseUrl = this.server.url
  })

  After(async function ({ pickle, result }) {
    try {
      const failed = result?.status === Status.FAILED || result?.status === 'FAILED'

      if (failed && this.page) {
        const screenshotPath = await getScenarioArtifactPath(this, pickle?.name, 'failure.png')
        const screenshot = await this.page.screenshot({ path: screenshotPath, fullPage: true }).catch(() => null)
        if (screenshot) {
          await this.attach(screenshot, 'image/png')
        }
      }

      if (this.page && (failed || this.consoleErrors.length > 0 || this.pageErrors.length > 0)) {
        const diagnostics = {
          browserMode: this.browserMode,
          eguiError: await readEguiError(this.page).catch(() => null),
          consoleErrors: this.consoleErrors,
          pageErrors: this.pageErrors,
        }
        await this.attach(JSON.stringify(diagnostics, null, 2), 'application/json')
      }
    } finally {
      await closeWorldBrowser(this)
      await shutdownSharedWebServer()
      this.resetRuntimeState()
    }
  })
})
