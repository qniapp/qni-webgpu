const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

test('CI workflow runs static web suites against an explicitly managed external server with readiness polling', async () => {
  const source = await fs.readFile(path.join(__dirname, '..', '.github', 'workflows', 'ci.yml'), 'utf8')

  assert.match(source, /QNI_EGUI_WEB_EXTERNAL_SERVER=1/)
  assert.match(source, /Timed out waiting for static egui-web server/)
})

test('CI workflow shards legacy Playwright after building and uploading one static web dist artifact', async () => {
  const source = await fs.readFile(path.join(__dirname, '..', '.github', 'workflows', 'ci.yml'), 'utf8')

  assert.match(source, /web-build:/)
  assert.match(source, /name: Upload web dist artifact/)
  assert.match(source, /name: Download web dist artifact/)
  assert.match(source, /web-legacy-shard-1:/)
  assert.match(source, /web-legacy-shard-2:/)
  assert.match(source, /pnpm exec playwright test --shard=1\/2/)
  assert.match(source, /pnpm exec playwright test --shard=2\/2/)
})
