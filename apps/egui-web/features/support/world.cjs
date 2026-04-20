const os = require('node:os')
const path = require('node:path')
const { setWorldConstructor } = require('@cucumber/cucumber')

const { getWebServerConfig } = require('../../test-support/web-server.cjs')
const { STANDARD_BROWSER_MODE } = require('./browser.cjs')

const registerSupportCode = (register) => {
  try {
    register()
  } catch (error) {
    if (!String(error).includes("isn't running")) {
      throw error
    }
  }
}

class EguiWorld {
  constructor({ attach, log, link, parameters }) {
    this.attach = attach
    this.log = log
    this.link = link
    this.parameters = parameters
    this.baseUrl = getWebServerConfig().url
    this.artifactDir = path.join(os.tmpdir(), 'qni-egui-web-cucumber')
    this.resetRuntimeState()
  }

  resetRuntimeState() {
    this.browserMode = STANDARD_BROWSER_MODE
    this.browser = null
    this.context = null
    this.page = null
    this.server = null
    this.consoleErrors = []
    this.pageErrors = []
    this.currentScenarioName = null
  }

  startScenario(name) {
    this.currentScenarioName = name || 'scenario'
    this.consoleErrors = []
    this.pageErrors = []
  }

  setBrowserMode(mode) {
    this.browserMode = mode
  }
}

registerSupportCode(() => {
  setWorldConstructor(EguiWorld)
})

module.exports = {
  EguiWorld,
}
