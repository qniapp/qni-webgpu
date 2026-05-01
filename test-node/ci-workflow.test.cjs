const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

test('CI workflow runs static web suites against an explicitly managed external server with readiness polling', async () => {
  const source = await fs.readFile(path.join(__dirname, '..', '.github', 'workflows', 'ci.yml'), 'utf8')

  assert.match(source, /QNI_EGUI_WEB_EXTERNAL_SERVER=1/)
  assert.match(source, /Timed out waiting for static egui-web server/)
})

test('CI workflow starts one static server for both web BDD and legacy suites', async () => {
  const source = await fs.readFile(path.join(__dirname, '..', '.github', 'workflows', 'ci.yml'), 'utf8')

  const serverStarts = source.match(/python3 -m http\.server 4174/g) || []
  assert.equal(serverStarts.length, 1)
  assert.match(source, /pnpm run test:bdd[\s\S]*pnpm run test:pw-legacy/)
})
