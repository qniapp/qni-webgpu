const PLAYWRIGHT_EXTERNAL_SERVER_ENV = 'QNI_EGUI_WEB_EXTERNAL_SERVER'
const PLAYWRIGHT_BASE_URL_ENV = 'QNI_EGUI_WEB_BASE_URL'
const DEFAULT_WEB_SERVER_URL = 'http://127.0.0.1:4174'
const DEFAULT_WEB_SERVER_TIMEOUT_MS = 180_000

const getWebServerConfig = ({ env = process.env } = {}) => {
  if (env[PLAYWRIGHT_EXTERNAL_SERVER_ENV] === '1') {
    return {
      url: env[PLAYWRIGHT_BASE_URL_ENV] || DEFAULT_WEB_SERVER_URL,
      timeout: DEFAULT_WEB_SERVER_TIMEOUT_MS,
      reuseExistingServer: true,
      external: true,
    }
  }

  return {
    command: 'env -u NO_COLOR TRUNK_COLOR=never trunk serve --address 127.0.0.1 --port 4174 --no-autoreload',
    url: DEFAULT_WEB_SERVER_URL,
    timeout: DEFAULT_WEB_SERVER_TIMEOUT_MS,
    reuseExistingServer: true,
  }
}

const getPlaywrightBaseUrl = ({ env = process.env } = {}) => getWebServerConfig({ env }).url

const getPlaywrightWebServerConfig = ({ env = process.env } = {}) =>
  env[PLAYWRIGHT_EXTERNAL_SERVER_ENV] === '1' ? undefined : getWebServerConfig({ env })

module.exports = {
  getWebServerConfig,
  getPlaywrightBaseUrl,
  getPlaywrightWebServerConfig,
  PLAYWRIGHT_EXTERNAL_SERVER_ENV,
  PLAYWRIGHT_BASE_URL_ENV,
}
