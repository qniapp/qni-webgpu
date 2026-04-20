const getWebServerConfig = () => ({
  command: 'env -u NO_COLOR TRUNK_COLOR=never trunk serve --address 127.0.0.1 --port 4174 --no-autoreload',
  url: 'http://127.0.0.1:4174',
  timeout: 180_000,
  reuseExistingServer: true,
})

module.exports = {
  getWebServerConfig,
}
