const { execFileSync } = require('node:child_process')

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

module.exports = {
  resolvePlaywrightBrowserExecutable,
}
