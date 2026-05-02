const test = require('node:test')
const assert = require('node:assert/strict')
require('ts-node/register/transpile-only')
const fs = require('node:fs/promises')
const path = require('node:path')

const { chromium } = require('playwright')
const {
  getStandardWebGpuLaunchOptions,
} = require('../test-support/browser-launch.ts')
const {
  getWebServerConfig,
  getPlaywrightBaseUrl,
  PLAYWRIGHT_EXTERNAL_SERVER_ENV,
  PLAYWRIGHT_BASE_URL_ENV,
} = require('../test-support/web-server.ts')

const rootDir = path.join(__dirname, '..')
const repoRoot = path.join(rootDir, '..', '..')

const loadConfig = (env = process.env) => {
  const configPath = require.resolve('../playwright.config.ts')
  delete require.cache[configPath]

  const previousExternal = process.env[PLAYWRIGHT_EXTERNAL_SERVER_ENV]
  const previousBaseUrl = process.env[PLAYWRIGHT_BASE_URL_ENV]
  const previousCi = process.env.CI

  if (env[PLAYWRIGHT_EXTERNAL_SERVER_ENV] === undefined) {
    delete process.env[PLAYWRIGHT_EXTERNAL_SERVER_ENV]
  } else {
    process.env[PLAYWRIGHT_EXTERNAL_SERVER_ENV] = env[PLAYWRIGHT_EXTERNAL_SERVER_ENV]
  }

  if (env[PLAYWRIGHT_BASE_URL_ENV] === undefined) {
    delete process.env[PLAYWRIGHT_BASE_URL_ENV]
  } else {
    process.env[PLAYWRIGHT_BASE_URL_ENV] = env[PLAYWRIGHT_BASE_URL_ENV]
  }

  if (env.CI === undefined) {
    delete process.env.CI
  } else {
    process.env.CI = env.CI
  }

  try {
    const configModule = require('../playwright.config.ts')
    return configModule.default || configModule
  } finally {
    if (previousExternal === undefined) {
      delete process.env[PLAYWRIGHT_EXTERNAL_SERVER_ENV]
    } else {
      process.env[PLAYWRIGHT_EXTERNAL_SERVER_ENV] = previousExternal
    }

    if (previousBaseUrl === undefined) {
      delete process.env[PLAYWRIGHT_BASE_URL_ENV]
    } else {
      process.env[PLAYWRIGHT_BASE_URL_ENV] = previousBaseUrl
    }

    if (previousCi === undefined) {
      delete process.env.CI
    } else {
      process.env.CI = previousCi
    }

    delete require.cache[configPath]
  }
}

test('playwright config is TypeScript without a compatibility wrapper', async () => {
  await assert.doesNotReject(() => fs.access(path.join(rootDir, 'playwright.config.ts')))
  await assert.rejects(() => fs.access(path.join(rootDir, 'playwright.config.cjs')), /ENOENT/)

  const docs = await fs.readFile(path.join(repoRoot, 'docs', 'egui-web.md'), 'utf8')
  assert.match(docs, /playwright\.config\.ts/)
  assert.doesNotMatch(docs, /playwright\.config\.cjs/)
})

test('playwright config uses the shared browser and web server policies', () => {
  const config = loadConfig()
  const expectedBrowser = getStandardWebGpuLaunchOptions({
    env: process.env,
    defaultPath: chromium.executablePath(),
  })

  const expectedWebServer = getWebServerConfig()

  assert.equal(config.fullyParallel, true)
  assert.equal(config.use.baseURL, expectedWebServer.url)
  assert.equal(config.use.headless, expectedBrowser.headless)
  assert.deepEqual(config.use.launchOptions, {
    executablePath: expectedBrowser.executablePath,
    args: expectedBrowser.args,
  })
  assert.deepEqual(config.webServer, expectedWebServer)
})

test('playwright config uses a bounded multi-worker count on CI', () => {
  const env = {
    ...process.env,
    CI: '1',
  }
  const config = loadConfig(env)

  assert.equal(config.workers, 6)
})

test('playwright config can reuse an externally managed egui-web server', () => {
  const env = {
    ...process.env,
    [PLAYWRIGHT_EXTERNAL_SERVER_ENV]: '1',
    [PLAYWRIGHT_BASE_URL_ENV]: 'http://127.0.0.1:5999',
  }
  const config = loadConfig(env)

  assert.equal(config.use.baseURL, getPlaywrightBaseUrl({ env }))
  assert.equal(config.webServer, undefined)
})
