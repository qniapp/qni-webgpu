const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

const rootDir = path.join(__dirname, '..')

const readPackageJson = async () => {
  const packageJsonPath = path.join(rootDir, 'package.json')
  return JSON.parse(await fs.readFile(packageJsonPath, 'utf8'))
}

test('package scripts add bdd and keep legacy Playwright as the primary test command', async () => {
  const pkg = await readPackageJson()

  assert.equal(pkg.scripts.test, 'playwright test')
  assert.equal(pkg.scripts['test:pw-legacy'], 'playwright test')
  assert.equal(pkg.scripts['test:bdd'], 'cucumber-js --config cucumber.cjs')
  assert.equal(pkg.scripts['test:preflight'], 'node --test test-node/*.test.cjs')
  assert.match(pkg.devDependencies['@cucumber/cucumber'], /^\^\d+/)
})

test('cucumber config only targets markdown feature files and support globs', () => {
  const config = require('../cucumber.cjs')

  assert.deepEqual(config.paths, ['features/**/*.feature.md'])
  assert.deepEqual([...config.require].sort(), [
    'features/step_definitions/**/*.cjs',
    'features/support/**/*.cjs',
  ].sort())
  assert.equal(config.publishQuiet, true)
  assert.equal(config.failFast, true)
})

test('support scaffolding loads and reuses the shared Task 1 browser and server policies', () => {
  const sharedBrowser = require('../test-support/browser-launch.cjs')
  const sharedServer = require('../test-support/web-server.cjs')
  const browser = require('../features/support/browser.cjs')
  const server = require('../features/support/server.cjs')
  const world = require('../features/support/world.cjs')
  const helpers = require('../features/support/egui-helpers.cjs')

  assert.doesNotThrow(() => require('../features/support/hooks.cjs'))
  assert.equal(browser.getStandardWebGpuLaunchOptions, sharedBrowser.getStandardWebGpuLaunchOptions)
  assert.equal(browser.getPlainChromiumLaunchOptions, sharedBrowser.getPlainChromiumLaunchOptions)
  assert.equal(server.getSharedWebServerConfig, sharedServer.getWebServerConfig)
  assert.equal(typeof browser.launchBrowserForMode, 'function')
  assert.equal(typeof server.ensureSharedWebServer, 'function')
  assert.equal(typeof server.shutdownSharedWebServer, 'function')
  assert.equal(typeof world.EguiWorld, 'function')
  assert.equal(typeof helpers.waitForAppReady, 'function')
  assert.equal(typeof helpers.readEguiError, 'function')
})
