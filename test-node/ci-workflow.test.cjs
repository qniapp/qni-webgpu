const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

test('CI workflow runs static web suites against an explicitly managed external server with readiness polling', async () => {
  const source = await fs.readFile(path.join(__dirname, '..', '.github', 'workflows', 'ci.yml'), 'utf8')

  assert.match(source, /QNI_EGUI_WEB_EXTERNAL_SERVER=1/)
  assert.match(source, /Timed out waiting for static egui-web server/)
})
