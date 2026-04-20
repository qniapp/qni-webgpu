const { chromium } = require('playwright')
const {
  getStandardWebGpuLaunchOptions,
  getPlainChromiumLaunchOptions,
} = require('../../test-support/browser-launch.cjs')

const STANDARD_BROWSER_MODE = 'standard-webgpu'
const PLAIN_BROWSER_MODE = 'plain-chromium'
const DEFAULT_VIEWPORT = { width: 1000, height: 800 }

const closeWorldBrowser = async (world) => {
  await world.page?.close().catch(() => {})
  await world.context?.close().catch(() => {})
  await world.browser?.close().catch(() => {})

  world.page = null
  world.context = null
  world.browser = null
}

const getBrowserLaunchOptions = (mode = STANDARD_BROWSER_MODE, {
  env = process.env,
  defaultPath = chromium.executablePath(),
  commandLookup,
  headless,
} = {}) => {
  if (mode === STANDARD_BROWSER_MODE) {
    return getStandardWebGpuLaunchOptions({ env, defaultPath, commandLookup, headless })
  }

  if (mode === PLAIN_BROWSER_MODE) {
    return getPlainChromiumLaunchOptions({ env, defaultPath, headless })
  }

  throw new Error(`Unknown egui-web browser mode: ${mode}`)
}

const launchBrowserForMode = async (mode = STANDARD_BROWSER_MODE, options = {}) =>
  chromium.launch(getBrowserLaunchOptions(mode, options))

const openPageForMode = async (world, mode = STANDARD_BROWSER_MODE, {
  contextOptions = {},
  launchOptions = {},
} = {}) => {
  await closeWorldBrowser(world)

  world.setBrowserMode(mode)
  world.browser = await launchBrowserForMode(mode, launchOptions)
  world.context = await world.browser.newContext({
    viewport: DEFAULT_VIEWPORT,
    ...contextOptions,
  })
  world.page = await world.context.newPage()
  world.page.on('console', (message) => {
    if (message.type() === 'error') {
      world.consoleErrors.push(message.text())
    }
  })
  world.page.on('pageerror', (error) => {
    world.pageErrors.push(error.message)
  })
  return world.page
}

module.exports = {
  STANDARD_BROWSER_MODE,
  PLAIN_BROWSER_MODE,
  DEFAULT_VIEWPORT,
  getStandardWebGpuLaunchOptions,
  getPlainChromiumLaunchOptions,
  getBrowserLaunchOptions,
  launchBrowserForMode,
  openPageForMode,
  closeWorldBrowser,
}
