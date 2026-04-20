const { setWorldConstructor: defaultSetWorldConstructor } = require('@cucumber/cucumber')

const { getWebServerConfig } = require('../../test-support/web-server.cjs')
const { DEFAULT_ARTIFACT_DIR } = require('./egui-helpers.cjs')
const { STANDARD_BROWSER_MODE } = require('./browser.cjs')

class EguiWorld {
  constructor({ attach, log, link, parameters }) {
    this.attach = attach
    this.log = log
    this.link = link
    this.parameters = parameters
    this.baseUrl = getWebServerConfig().url
    this.artifactDir = DEFAULT_ARTIFACT_DIR
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

const registerWorld = ({ setWorldConstructor = defaultSetWorldConstructor } = {}) => {
  setWorldConstructor(EguiWorld)
}

module.exports = {
  EguiWorld,
  registerWorld,
}
