const PLAYWRIGHT_EXTERNAL_SERVER_ENV = 'QNI_EGUI_WEB_EXTERNAL_SERVER'
const PLAYWRIGHT_BASE_URL_ENV = 'QNI_EGUI_WEB_BASE_URL'

const getWebServerConfig = () => ({
  command: 'env -u NO_COLOR TRUNK_COLOR=never trunk serve --address 127.0.0.1 --port 4174 --no-autoreload',
  url: 'http://127.0.0.1:4174',
  timeout: 180_000,
  reuseExistingServer: true,
})

const getPlaywrightBaseUrl = ({ env = process.env } = {}) =>
  env[PLAYWRIGHT_BASE_URL_ENV] || getWebServerConfig().url

const getPlaywrightWebServerConfig = ({ env = process.env } = {}) =>
  env[PLAYWRIGHT_EXTERNAL_SERVER_ENV] === '1' ? undefined : getWebServerConfig()

module.exports = {
  getWebServerConfig,
  getPlaywrightBaseUrl,
  getPlaywrightWebServerConfig,
  PLAYWRIGHT_EXTERNAL_SERVER_ENV,
  PLAYWRIGHT_BASE_URL_ENV,
}
