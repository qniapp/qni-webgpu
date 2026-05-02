// Backward-compatible wrapper so existing Playwright imports keep using the shared policy.
require('ts-node/register/transpile-only')
module.exports = require('./test-support/browser-launch.ts')
