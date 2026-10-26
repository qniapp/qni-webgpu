import { execFileSync } from 'node:child_process'
import type { LaunchOptions } from 'playwright'

type CommandLookup = (name: string) => string | null
type BrowserLaunchRequest = {
  env?: NodeJS.ProcessEnv
  defaultPath?: string
  commandLookup?: CommandLookup
  headless?: boolean
}

const WEBGPU_COMMON_ARGS = [
  '--enable-features=WebGPU,WebGPUDeveloperFeatures,WebGPUService,Vulkan',
  '--enable-unsafe-webgpu',
  '--enable-dawn-features=allow_unsafe_apis,enable_immediate_error_handling',
  '--ignore-gpu-blocklist',
  '--disable-gpu-sandbox',
  '--no-sandbox',
  '--use-gl=angle',
]

export const STANDARD_WEBGPU_ARGS = [
  ...WEBGPU_COMMON_ARGS,
  '--use-angle=swiftshader',
  '--use-vulkan=swiftshader',
]

export const PLAIN_CHROMIUM_ARGS = ['--disable-gpu', '--disable-software-rasterizer']

export const AGENT_VISUAL_WEBGPU_ARGS = [
  '--enable-features=WebGPU,WebGPUDeveloperFeatures,WebGPUService,Vulkan',
  '--enable-unsafe-webgpu',
  '--ignore-gpu-blocklist',
]

const findCommand = (name: string): string | null => {
  try {
    return execFileSync('sh', ['-lc', `command -v ${name}`], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    }).trim() || null
  } catch {
    return null
  }
}

export const resolvePlaywrightBrowserExecutable = ({
  env = process.env,
  defaultPath,
  commandLookup = findCommand,
}: BrowserLaunchRequest = {}): string | undefined => {
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

export const getStandardWebGpuLaunchOptions = ({
  env = process.env,
  defaultPath,
  commandLookup = findCommand,
  headless = env.HEADLESS !== '0',
}: BrowserLaunchRequest = {}): LaunchOptions => {
  const backend = env.QNI_WEB_GPU_BACKEND ?? 'swiftshader'
  if (backend !== 'swiftshader' && backend !== 'vulkan') {
    throw new Error('QNI_WEB_GPU_BACKEND must be swiftshader or vulkan')
  }
  return {
    headless,
    executablePath: resolvePlaywrightBrowserExecutable({ env, defaultPath, commandLookup }),
    args: backend === 'vulkan'
      ? [...WEBGPU_COMMON_ARGS, '--use-angle=vulkan', '--use-vulkan=native']
      : [...STANDARD_WEBGPU_ARGS],
  }
}

export const getPlainChromiumLaunchOptions = ({
  env = process.env,
  defaultPath,
  commandLookup = findCommand,
  headless = true,
}: BrowserLaunchRequest = {}): LaunchOptions => ({
  headless,
  executablePath: resolvePlaywrightBrowserExecutable({ env, defaultPath, commandLookup }),
  args: [...PLAIN_CHROMIUM_ARGS],
})

export const getAgentVisualLaunchOptions = ({
  env = process.env,
  defaultPath,
  commandLookup = findCommand,
  headless = env.HEADLESS === '1',
}: BrowserLaunchRequest = {}): LaunchOptions => ({
  headless,
  executablePath: resolvePlaywrightBrowserExecutable({ env, defaultPath, commandLookup }),
  args: [...AGENT_VISUAL_WEBGPU_ARGS],
})
