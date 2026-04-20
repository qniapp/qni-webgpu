const {
  After: defaultAfter,
  AfterAll: defaultAfterAll,
  Before: defaultBefore,
  BeforeAll: defaultBeforeAll,
  Status,
} = require('@cucumber/cucumber')

const { closeWorldBrowser } = require('./browser.cjs')
const { readEguiError, getScenarioArtifactPath } = require('./egui-helpers.cjs')
const { ensureSharedWebServer, getSharedWebServerConfig, shutdownSharedWebServer } = require('./server.cjs')

const registerHooks = ({
  BeforeAll = defaultBeforeAll,
  Before = defaultBefore,
  After = defaultAfter,
  AfterAll = defaultAfterAll,
  Status: statusEnum = Status,
  ensureSharedWebServer: ensureServer = ensureSharedWebServer,
  shutdownSharedWebServer: shutdownServer = shutdownSharedWebServer,
  closeWorldBrowser: closeBrowser = closeWorldBrowser,
  readEguiError: readError = readEguiError,
  getScenarioArtifactPath: getArtifactPath = getScenarioArtifactPath,
  getSharedWebServerConfig: getServerConfig = getSharedWebServerConfig,
} = {}) => {
  let sharedServer = null

  BeforeAll(async function () {
    sharedServer = await ensureServer()
  })

  Before(function ({ pickle }) {
    this.startScenario(pickle?.name)
    this.server = sharedServer
    this.baseUrl = sharedServer?.url || getServerConfig().url
  })

  After(async function ({ pickle, result }) {
    try {
      const failed = result?.status === statusEnum.FAILED || result?.status === 'FAILED'

      if (failed && this.page) {
        const screenshotPath = await getArtifactPath(this, pickle?.name, 'failure.png')
        const screenshot = await this.page.screenshot({ path: screenshotPath, fullPage: true }).catch(() => null)
        if (screenshot) {
          await this.attach(screenshot, 'image/png')
        }
      }

      if (this.page && (failed || this.consoleErrors.length > 0 || this.pageErrors.length > 0)) {
        const diagnostics = {
          browserMode: this.browserMode,
          eguiError: await readError(this.page).catch(() => null),
          consoleErrors: this.consoleErrors,
          pageErrors: this.pageErrors,
        }
        await this.attach(JSON.stringify(diagnostics, null, 2), 'application/json')
      }
    } finally {
      await closeBrowser(this)
      this.resetRuntimeState()
      this.baseUrl = sharedServer?.url || getServerConfig().url
    }
  })

  AfterAll(async function () {
    try {
      await shutdownServer()
    } finally {
      sharedServer = null
    }
  })
}

module.exports = {
  registerHooks,
}
