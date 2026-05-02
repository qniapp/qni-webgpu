const test = require('node:test')
const assert = require('node:assert/strict')
const { spawnSync } = require('node:child_process')
require('ts-node/register/transpile-only')
const { loadConfiguration } = require('@cucumber/cucumber/api')
const fs = require('node:fs/promises')
const os = require('node:os')
const path = require('node:path')

const rootDir = path.join(__dirname, '..')
const supportDir = path.join(rootDir, 'features', 'support')
const pnpmCommand = process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm'
const CUCUMBER_SMOKE_TIMEOUT_MS = 20_000

const readPackageJson = async () => {
  const packageJsonPath = path.join(rootDir, 'package.json')
  return JSON.parse(await fs.readFile(packageJsonPath, 'utf8'))
}

const readTsConfig = async () => {
  const tsconfigPath = path.join(rootDir, 'tsconfig.json')
  return JSON.parse(await fs.readFile(tsconfigPath, 'utf8'))
}

const readSupportSource = async (fileName) => fs.readFile(path.join(supportDir, fileName), 'utf8')

const parseMessageOutput = async (messagePath) => {
  const lines = (await fs.readFile(messagePath, 'utf8'))
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)

  return lines.map((line) => JSON.parse(line))
}

const loadCucumberConfig = (env = process.env) => {
  const configPath = require.resolve('../cucumber.cjs')
  delete require.cache[configPath]

  const previousCi = process.env.CI
  if (env.CI === undefined) {
    delete process.env.CI
  } else {
    process.env.CI = env.CI
  }

  try {
    return require('../cucumber.cjs').default
  } finally {
    if (previousCi === undefined) {
      delete process.env.CI
    } else {
      process.env.CI = previousCi
    }
    delete require.cache[configPath]
  }
}

const writeTempSmokeFixture = async (featureText) => {
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'egui-web-cucumber-smoke-'))
  const featurePath = path.join(tempDir, 'smoke.feature.md')
  const stepsPath = path.join(tempDir, 'smoke.steps.ts')
  const messagePath = path.join(tempDir, 'messages.ndjson')

  await fs.writeFile(featurePath, featureText)
  await fs.writeFile(
    stepsPath,
    [
      `const { Given } = require(${JSON.stringify(require.resolve('@cucumber/cucumber'))})`,
      "enum SmokeKind { Noop = 'noop' }",
      "Given('a smoke noop step', function () { void SmokeKind.Noop })",
      '',
    ].join('\n')
  )

  return { tempDir, featurePath, stepsPath, messagePath }
}

test('package scripts add bdd and keep legacy Playwright as the primary test command', async () => {
  const pkg = await readPackageJson()

  assert.equal(pkg.scripts.test, 'playwright test')
  assert.equal(pkg.scripts['test:pw-legacy'], 'playwright test')
  assert.equal(pkg.scripts['test:bdd'], 'cucumber-js --config cucumber.cjs')
  assert.equal(pkg.scripts.typecheck, 'tsc --noEmit')
  assert.equal(pkg.scripts['test:preflight'], 'pnpm run typecheck && node --test test-node/*.test.cjs')
  assert.match(pkg.devDependencies['@cucumber/cucumber'], /^\^\d+/)
  assert.match(pkg.devDependencies['@types/node'], /^\^\d+/)
  assert.match(pkg.devDependencies['ts-node'], /^\^\d+/)
  assert.match(pkg.devDependencies.typescript, /^\^\d+/)
})

test('typescript config type-checks cucumber glue without emitting files', async () => {
  const tsconfig = await readTsConfig()

  assert.equal(tsconfig.compilerOptions.noEmit, true)
  assert.equal(tsconfig.compilerOptions.rootDir, '.')
  assert.equal(tsconfig.compilerOptions.module, 'CommonJS')
  assert.equal(tsconfig.compilerOptions.strict, true)
  assert.ok(tsconfig.compilerOptions.types.includes('node'))
  assert.ok(tsconfig.include.includes('features/**/*.ts'))
  assert.ok(tsconfig.include.includes('features/**/*.d.ts'))
})

test('cucumber CLI resolves the default profile with TypeScript support', async () => {
  const { useConfiguration, runConfiguration } = await loadConfiguration({ file: 'cucumber.cjs' })

  assert.deepEqual(useConfiguration.paths, ['features/**/*.feature.md'])
  assert.deepEqual(runConfiguration.support.requireModules, ['ts-node/register'])
  assert.deepEqual([...runConfiguration.support.requirePaths].sort(), [
    'features/step_definitions/**/*.cjs',
    'features/step_definitions/**/*.ts',
    'features/support/bootstrap.cjs',
  ].sort())
  assert.equal(useConfiguration.publish, false)
  assert.equal(useConfiguration.failFast, true)
})

test('cucumber config only targets markdown feature files and uses explicit support bootstrap', () => {
  const config = loadCucumberConfig()

  assert.deepEqual(config.paths, ['features/**/*.feature.md'])
  assert.deepEqual(config.requireModule, ['ts-node/register'])
  assert.deepEqual([...config.require].sort(), [
    'features/step_definitions/**/*.cjs',
    'features/step_definitions/**/*.ts',
    'features/support/bootstrap.cjs',
  ].sort())
  assert.equal(config.publishQuiet, true)
  assert.equal(config.failFast, true)
})

test('cucumber config uses bounded scenario parallelism on CI', () => {
  const config = loadCucumberConfig({ ...process.env, CI: '1' })

  assert.equal(config.parallel, 2)
})

test('support modules expose explicit registration hooks without runtime message sniffing', async () => {
  const hooks = require('../features/support/hooks.cjs')
  const world = require('../features/support/world.ts')

  assert.equal(typeof hooks.registerHooks, 'function')
  assert.equal(typeof world.registerWorld, 'function')
  await assert.doesNotReject(() => fs.access(path.join(supportDir, 'bootstrap.cjs')))

  const [hooksSource, worldSource] = await Promise.all([
    readSupportSource('hooks.cjs'),
    readSupportSource('world.ts'),
  ])

  assert.doesNotMatch(hooksSource, /isn['’]?t running/)
  assert.doesNotMatch(worldSource, /isn['’]?t running/)
})

test('support hooks keep shared server lifecycle at run scope while resetting browser state per scenario', async () => {
  const hooks = require('../features/support/hooks.cjs')
  const registrations = {}
  const calls = []

  hooks.registerHooks({
    BeforeAll: (options, callback) => {
      registrations.beforeAllOptions = options
      registrations.beforeAll = callback
    },
    Before: (callback) => {
      registrations.before = callback
    },
    After: (callback) => {
      registrations.after = callback
    },
    AfterAll: (options, callback) => {
      registrations.afterAllOptions = options
      registrations.afterAll = callback
    },
    Status: { FAILED: 'FAILED' },
    ensureSharedWebServer: async () => {
      calls.push('ensure-server')
      return { url: 'http://127.0.0.1:4174', managed: true }
    },
    shutdownSharedWebServer: async () => {
      calls.push('shutdown-server')
    },
    closeWorldBrowser: async () => {
      calls.push('close-browser')
    },
    readEguiError: async () => null,
    getScenarioArtifactPath: async () => '/tmp/failure.png',
    getSharedWebServerConfig: () => ({ url: 'http://127.0.0.1:4174', timeout: 123_456 }),
  })

  assert.equal(typeof registrations.beforeAll, 'function')
  assert.equal(typeof registrations.before, 'function')
  assert.equal(typeof registrations.after, 'function')
  assert.equal(typeof registrations.afterAll, 'function')
  assert.deepEqual(registrations.beforeAllOptions, { timeout: 123_456 })
  assert.deepEqual(registrations.afterAllOptions, { timeout: 123_456 })

  const world = {
    page: null,
    consoleErrors: [],
    pageErrors: [],
    async attach() {},
    startScenario(name) {
      calls.push(`start:${name}`)
    },
    resetRuntimeState() {
      calls.push('reset-world')
    },
  }

  await registrations.beforeAll.call({})
  await registrations.before.call(world, { pickle: { name: 'smoke scenario' } })
  await registrations.after.call(world, {
    pickle: { name: 'smoke scenario' },
    result: { status: 'PASSED' },
  })
  await registrations.afterAll.call({})

  assert.equal(world.baseUrl, 'http://127.0.0.1:4174')
  assert.deepEqual(calls, [
    'ensure-server',
    'start:smoke scenario',
    'close-browser',
    'reset-world',
    'shutdown-server',
  ])
})

test('cucumber dry-run smoke plans exactly one selected markdown scenario without relying on formatter text', async (t) => {
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
      '--format',
      `message:${fixture.messagePath}`,
      '--require',
      fixture.stepsPath,
      '--name',
      '^runner loads config and support$',
      fixture.featurePath,
    ],
    {
      cwd: rootDir,
      encoding: 'utf8',
      timeout: CUCUMBER_SMOKE_TIMEOUT_MS,
    }
  )

  assert.ifError(result.error)
  assert.equal(result.signal, null, `stdout:\n${result.stdout}\nstderr:\n${result.stderr}`)
  assert.equal(result.status, 0, `stderr:\n${result.stderr}\nstdout:\n${result.stdout}`)

  const messages = await parseMessageOutput(fixture.messagePath)
  const testCaseStartedCount = messages.filter((message) => message.testCaseStarted).length

  assert.equal(testCaseStartedCount, 1, `stdout:\n${result.stdout}\nstderr:\n${result.stderr}`)
})

test('support scaffolding loads and reuses the shared Task 1 browser and server policies', () => {
  const sharedBrowser = require('../test-support/browser-launch.cjs')
  const sharedServer = require('../test-support/web-server.cjs')
  const browser = require('../features/support/browser.ts')
  const server = require('../features/support/server.ts')
  const world = require('../features/support/world.ts')
  const helpers = require('../features/support/egui-helpers.cjs')

  assert.equal(browser.getStandardWebGpuLaunchOptions, sharedBrowser.getStandardWebGpuLaunchOptions)
  assert.equal(browser.getPlainChromiumLaunchOptions, sharedBrowser.getPlainChromiumLaunchOptions)
  assert.equal(server.getSharedWebServerConfig, sharedServer.getWebServerConfig)
  assert.equal(typeof browser.launchBrowserForMode, 'function')
  assert.equal(typeof server.ensureSharedWebServer, 'function')
  assert.equal(typeof server.shutdownSharedWebServer, 'function')
  assert.equal(typeof world.EguiWorld, 'function')
  assert.equal(typeof helpers.waitForAppReady, 'function')
  assert.equal(typeof helpers.waitForStartupReady, 'function')
  assert.equal(typeof helpers.readEguiError, 'function')
})

test('cucumber world uses an externally managed base URL when configured', () => {
  const worldModulePath = require.resolve('../features/support/world.ts')
  delete require.cache[worldModulePath]

  const previousExternal = process.env.QNI_EGUI_WEB_EXTERNAL_SERVER
  const previousBaseUrl = process.env.QNI_EGUI_WEB_BASE_URL
  process.env.QNI_EGUI_WEB_EXTERNAL_SERVER = '1'
  process.env.QNI_EGUI_WEB_BASE_URL = 'http://127.0.0.1:5999'

  try {
    const { EguiWorld } = require('../features/support/world.ts')
    const world = new EguiWorld({
      attach: async () => {},
      log: () => {},
      link: () => {},
      parameters: {},
    })

    assert.equal(world.baseUrl, 'http://127.0.0.1:5999')
  } finally {
    if (previousExternal === undefined) {
      delete process.env.QNI_EGUI_WEB_EXTERNAL_SERVER
    } else {
      process.env.QNI_EGUI_WEB_EXTERNAL_SERVER = previousExternal
    }

    if (previousBaseUrl === undefined) {
      delete process.env.QNI_EGUI_WEB_BASE_URL
    } else {
      process.env.QNI_EGUI_WEB_BASE_URL = previousBaseUrl
    }

    delete require.cache[worldModulePath]
  }
})
