const test = require('node:test')
const assert = require('node:assert/strict')

const {
  getAgentVisualLaunchOptions,
  getPlainChromiumLaunchOptions,
  getStandardWebGpuLaunchOptions,
  resolvePlaywrightBrowserExecutable,
} = require('../test-support/browser-launch.ts')

const STANDARD_WEBGPU_ARGS = [
  '--enable-features=WebGPU,WebGPUDeveloperFeatures,WebGPUService,Vulkan',
  '--enable-unsafe-webgpu',
  '--enable-dawn-features=allow_unsafe_apis,enable_immediate_error_handling',
  '--ignore-gpu-blocklist',
  '--disable-gpu-sandbox',
  '--no-sandbox',
  '--use-gl=angle',
  '--use-angle=swiftshader',
  '--use-vulkan=swiftshader',
]

const PLAIN_CHROMIUM_ARGS = ['--disable-gpu', '--disable-software-rasterizer']
const AGENT_VISUAL_WEBGPU_ARGS = [
  '--enable-features=WebGPU,WebGPUDeveloperFeatures,WebGPUService,Vulkan',
  '--enable-unsafe-webgpu',
  '--ignore-gpu-blocklist',
]

test('shared browser executable resolution uses PLAYWRIGHT_CHROMIUM_PATH override first', () => {
  const actual = resolvePlaywrightBrowserExecutable({
    env: { PLAYWRIGHT_CHROMIUM_PATH: '/custom/browser' },
    defaultPath: '/bundled/chromium',
    commandLookup: () => null,
  })

  assert.equal(actual, '/custom/browser')
})

test('shared browser executable resolution falls back to the bundled default when no command is found', () => {
  const actual = resolvePlaywrightBrowserExecutable({
    env: {},
    defaultPath: '/bundled/chromium',
    commandLookup: () => null,
  })

  assert.equal(actual, '/bundled/chromium')
})

test('shared browser executable resolution preserves system Chrome priority', () => {
  const actual = resolvePlaywrightBrowserExecutable({
    env: {},
    defaultPath: '/bundled/chromium',
    commandLookup: (name: string) => {
      if (name === 'google-chrome') {
        return '/usr/bin/google-chrome'
      }
      if (name === 'chromium') {
        return '/usr/bin/chromium'
      }
      return null
    },
  })

  assert.equal(actual, '/usr/bin/google-chrome')
})

test('shared browser executable resolution supports chrome-only systems', () => {
  const actual = resolvePlaywrightBrowserExecutable({
    env: {},
    defaultPath: '/bundled/chromium',
    commandLookup: (name: string) => (name === 'chrome' ? '/usr/bin/chrome' : null),
  })

  assert.equal(actual, '/usr/bin/chrome')
})

test('shared standard WebGPU launch options preserve current flagged browser policy', () => {
  const actual = getStandardWebGpuLaunchOptions({
    env: { HEADLESS: '0' },
    defaultPath: '/bundled/chromium',
    commandLookup: (name: string) => (name === 'google-chrome-stable' ? '/usr/bin/google-chrome-stable' : null),
  })

  assert.deepEqual(actual, {
    headless: false,
    executablePath: '/usr/bin/google-chrome-stable',
    args: STANDARD_WEBGPU_ARGS,
  })
})

test('agent visual launch options keep screenshots inspectable', () => {
  const actual = getAgentVisualLaunchOptions({
    env: {},
    defaultPath: '/bundled/chromium',
    commandLookup: (name: string) => (name === 'google-chrome-stable' ? '/usr/bin/google-chrome-stable' : null),
  })

  assert.deepEqual(actual, {
    headless: false,
    executablePath: '/usr/bin/google-chrome-stable',
    args: AGENT_VISUAL_WEBGPU_ARGS,
  })
})

test('shared plain chromium launch options preserve current visible error policy', () => {
  const actual = getPlainChromiumLaunchOptions({
    env: { PLAYWRIGHT_CHROMIUM_PATH: '/custom/browser' },
    defaultPath: '/bundled/chromium',
  })

  assert.deepEqual(actual, {
    headless: true,
    executablePath: '/custom/browser',
    args: PLAIN_CHROMIUM_ARGS,
  })
})

test('shared plain chromium launch options fall back to Playwright bundled chromium when no system browser is found', () => {
  const actual = getPlainChromiumLaunchOptions({
    env: {},
    defaultPath: '/bundled/chromium',
    commandLookup: () => null,
  })

  assert.deepEqual(actual, {
    headless: true,
    executablePath: '/bundled/chromium',
    args: PLAIN_CHROMIUM_ARGS,
  })
})

export {}
