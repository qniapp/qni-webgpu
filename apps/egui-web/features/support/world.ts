import type { Browser, BrowserContext, Page } from 'playwright'
import type { DragPreviewZOrderSamples } from './support-types'

type WebServerConfig = {
  url: string
}

type WebServerSupport = {
  getWebServerConfig: (options?: { env?: NodeJS.ProcessEnv }) => WebServerConfig
}

type EguiHelpersSupport = {
  DEFAULT_ARTIFACT_DIR: string
}

type BrowserSupport = {
  STANDARD_BROWSER_MODE: string
}

type WorldAttach = (data: Buffer | string, mediaType?: string) => Promise<void> | void
type WorldLog = (text: string) => void
type WorldLink = (url: string, name?: string) => void

type WorldConstructorOptions = {
  attach: WorldAttach
  log: WorldLog
  link: WorldLink
  parameters: Record<string, unknown>
}

type SetWorldConstructor = (worldConstructor: typeof EguiWorld) => void

const { setWorldConstructor: defaultSetWorldConstructor } = require('@cucumber/cucumber') as {
  setWorldConstructor: SetWorldConstructor
}

const { getWebServerConfig } = require('../../test-support/web-server.cjs') as WebServerSupport
const { DEFAULT_ARTIFACT_DIR } = require('./egui-helpers.ts') as EguiHelpersSupport
const { STANDARD_BROWSER_MODE } = require('./browser.ts') as BrowserSupport

export class EguiWorld {
  attach: WorldAttach
  log: WorldLog
  link: WorldLink
  parameters: Record<string, unknown>
  baseUrl: string
  artifactDir: string
  browserMode: string = STANDARD_BROWSER_MODE
  browser: Browser | null = null
  context: BrowserContext | null = null
  page: Page | null = null
  server: { url: string; managed?: boolean } | null = null
  consoleErrors: string[] = []
  pageErrors: string[] = []
  currentScenarioName: string | null = null
  dragPreviewZOrder?: DragPreviewZOrderSamples

  constructor({ attach, log, link, parameters }: WorldConstructorOptions) {
    this.attach = attach
    this.log = log
    this.link = link
    this.parameters = parameters
    this.baseUrl = getWebServerConfig({ env: process.env }).url
    this.artifactDir = DEFAULT_ARTIFACT_DIR
    this.resetRuntimeState()
  }

  resetRuntimeState(): void {
    this.browserMode = STANDARD_BROWSER_MODE
    this.browser = null
    this.context = null
    this.page = null
    this.server = null
    this.consoleErrors = []
    this.pageErrors = []
    this.currentScenarioName = null
  }

  startScenario(name?: string | null): void {
    this.currentScenarioName = name || 'scenario'
    this.consoleErrors = []
    this.pageErrors = []
  }

  setBrowserMode(mode: string): void {
    this.browserMode = mode
  }
}

export const registerWorld = ({ setWorldConstructor = defaultSetWorldConstructor }: {
  setWorldConstructor?: SetWorldConstructor
} = {}): void => {
  setWorldConstructor(EguiWorld)
}
