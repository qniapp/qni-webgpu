const test = require('node:test')
const assert = require('node:assert/strict')
const { spawnSync } = require('node:child_process')
const fs = require('node:fs/promises')
const os = require('node:os')
const path = require('node:path')

const rootDir = path.join(__dirname, '..')
const pnpmCommand = process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm'

const readPackageJson = async () => {
  const packageJsonPath = path.join(rootDir, 'package.json')
  return JSON.parse(await fs.readFile(packageJsonPath, 'utf8'))
}

const writeTempSmokeFixture = async (featureText) => {
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'egui-web-cucumber-smoke-'))
  const featurePath = path.join(tempDir, 'smoke.feature.md')
  const stepsPath = path.join(tempDir, 'smoke.steps.cjs')

  await fs.writeFile(featurePath, featureText)
  await fs.writeFile(
    stepsPath,
    [
      "const { Given } = require('@cucumber/cucumber')",
      "Given('a smoke noop step', function () {})",
      '',
    ].join('\n')
  )

  return { tempDir, featurePath, stepsPath }
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

test('cucumber dry-run smoke uses a valid markdown-with-gherkin feature fixture', async (t) => {
  const fixture = await writeTempSmokeFixture([
    '# Feature: cucumber config smoke',
    '## Scenario: runner loads config and support',
    '- Given a smoke noop step',
    '',
  ].join('\n'))

  t.after(async () => {
    await fs.rm(fixture.tempDir, { recursive: true, force: true })
  })

  const result = spawnSync(
    pnpmCommand,
    [
      'exec',
      'cucumber-js',
      '--config',
      'cucumber.cjs',
      '--dry-run',
      '--require',
      fixture.stepsPath,
      fixture.featurePath,
    ],
    {
      cwd: rootDir,
      encoding: 'utf8',
    }
  )

  assert.ifError(result.error)
  assert.equal(result.status, 0, `stderr:\n${result.stderr}\nstdout:\n${result.stdout}`)
  assert.match(result.stdout, /1 scenario/, `stdout:\n${result.stdout}`)
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
