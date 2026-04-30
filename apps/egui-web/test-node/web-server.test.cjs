const test = require('node:test')
const assert = require('node:assert/strict')

const {
  getWebServerConfig,
  getPlaywrightBaseUrl,
  getPlaywrightWebServerConfig,
  PLAYWRIGHT_EXTERNAL_SERVER_ENV,
  PLAYWRIGHT_BASE_URL_ENV,
} = require('../test-support/web-server.cjs')

test('shared web server config preserves trunk serve contract', () => {
  assert.deepEqual(getWebServerConfig(), {
    command: 'env -u NO_COLOR TRUNK_COLOR=never trunk serve --address 127.0.0.1 --port 4174 --no-autoreload',
    url: 'http://127.0.0.1:4174',
    timeout: 180_000,
    reuseExistingServer: true,
  })
})

test('playwright web server helpers can target an externally managed egui-web server', () => {
  const env = {
    [PLAYWRIGHT_EXTERNAL_SERVER_ENV]: '1',
    [PLAYWRIGHT_BASE_URL_ENV]: 'http://127.0.0.1:5999',
  }

  assert.equal(getPlaywrightBaseUrl({ env }), 'http://127.0.0.1:5999')
  assert.equal(getPlaywrightWebServerConfig({ env }), undefined)
  assert.deepEqual(getPlaywrightWebServerConfig(), getWebServerConfig())
})
