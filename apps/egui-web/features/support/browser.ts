import { chromium } from 'playwright'
import type {
  Browser,
  BrowserContext,
  BrowserContextOptions,
  ConsoleMessage,
  LaunchOptions,
  Page,
} from 'playwright'
import type { EguiWorld } from './support-types'

type CommandLookup = (name: string) => string | null

type BrowserLaunchRequest = {
  env?: NodeJS.ProcessEnv
  defaultPath?: string
  commandLookup?: CommandLookup
  headless?: boolean
}

type BrowserLaunchSupport = {
  getStandardWebGpuLaunchOptions: (options?: BrowserLaunchRequest) => LaunchOptions
  getPlainChromiumLaunchOptions: (options?: BrowserLaunchRequest) => LaunchOptions
}

type WorldBrowserState = EguiWorld & {
  browser: Browser | null
  context: BrowserContext | null
  consoleErrors: string[]
  pageErrors: string[]
  setBrowserMode: (mode: string) => void
}

type OpenPageOptions = {
  contextOptions?: BrowserContextOptions
  launchOptions?: BrowserLaunchRequest
}

const {
  getStandardWebGpuLaunchOptions,
  getPlainChromiumLaunchOptions,
} = require('../../test-support/browser-launch.cjs') as BrowserLaunchSupport

export const STANDARD_BROWSER_MODE = 'standard-webgpu'
export const PLAIN_BROWSER_MODE = 'plain-chromium'
export const DEFAULT_VIEWPORT = { width: 1000, height: 800 } as const

export const closeWorldBrowser = async (world: WorldBrowserState): Promise<void> => {
  await world.page?.close().catch(() => {})
  await world.context?.close().catch(() => {})
  await world.browser?.close().catch(() => {})

  world.page = null
  world.context = null
  world.browser = null
}

export const getBrowserLaunchOptions = (
  mode: string = STANDARD_BROWSER_MODE,
  {
    env = process.env,
    defaultPath = chromium.executablePath(),
    commandLookup,
    headless,
  }: BrowserLaunchRequest = {}
): LaunchOptions => {
  if (mode === STANDARD_BROWSER_MODE) {
    return getStandardWebGpuLaunchOptions({ env, defaultPath, commandLookup, headless })
  }

  if (mode === PLAIN_BROWSER_MODE) {
    return getPlainChromiumLaunchOptions({ env, defaultPath, commandLookup, headless })
  }

  throw new Error(`Unknown egui-web browser mode: ${mode}`)
}

export const launchBrowserForMode = async (
  mode: string = STANDARD_BROWSER_MODE,
  options: BrowserLaunchRequest = {}
): Promise<Browser> => chromium.launch(getBrowserLaunchOptions(mode, options))

export const openPageForMode = async (
  world: WorldBrowserState,
  mode: string = STANDARD_BROWSER_MODE,
  { contextOptions = {}, launchOptions = {} }: OpenPageOptions = {}
): Promise<Page> => {
  await closeWorldBrowser(world)

  world.setBrowserMode(mode)
  world.browser = await launchBrowserForMode(mode, launchOptions)
  world.context = await world.browser.newContext({
    viewport: DEFAULT_VIEWPORT,
    ...contextOptions,
  })
  world.page = await world.context.newPage()
  world.page.on('console', (message: ConsoleMessage) => {
    if (message.type() === 'error') {
      world.consoleErrors.push(message.text())
    }
  })
  world.page.on('pageerror', (error: Error) => {
    world.pageErrors.push(error.message)
  })
  return world.page
}

export { getStandardWebGpuLaunchOptions, getPlainChromiumLaunchOptions }
