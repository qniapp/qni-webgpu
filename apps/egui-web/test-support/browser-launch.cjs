const { execFileSync } = require('node:child_process')

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

const findCommand = (name) => {
  try {
    return execFileSync('sh', ['-lc', `command -v ${name}`], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    }).trim() || null
  } catch {
    return null
  }
}

const resolvePlaywrightBrowserExecutable = ({
  env = process.env,
  defaultPath,
  commandLookup = findCommand,
} = {}) => {
  if (env.PLAYWRIGHT_CHROMIUM_PATH) {
    return env.PLAYWRIGHT_CHROMIUM_PATH
  }

  for (const name of ['google-chrome-stable', 'google-chrome', 'chromium', 'chromium-browser', 'chrome']) {
    const path = commandLookup(name)
    if (path) {
      return path
    }
  }

  return defaultPath
}

const getStandardWebGpuLaunchOptions = ({
  env = process.env,
  defaultPath,
  commandLookup = findCommand,
  headless = env.HEADLESS !== '0',
} = {}) => ({
  headless,
  executablePath: resolvePlaywrightBrowserExecutable({ env, defaultPath, commandLookup }),
  args: [...STANDARD_WEBGPU_ARGS],
})

const getPlainChromiumLaunchOptions = ({
  env = process.env,
  defaultPath,
  headless = true,
} = {}) => ({
  headless,
  executablePath: env.PLAYWRIGHT_CHROMIUM_PATH || defaultPath,
  args: [...PLAIN_CHROMIUM_ARGS],
})

module.exports = {
  STANDARD_WEBGPU_ARGS,
  PLAIN_CHROMIUM_ARGS,
  resolvePlaywrightBrowserExecutable,
  getStandardWebGpuLaunchOptions,
  getPlainChromiumLaunchOptions,
}
