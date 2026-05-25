const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

test('CI workflow runs static BDD against an explicitly managed external server with readiness polling', async () => {
  const source = await fs.readFile(path.join(__dirname, '..', '.github', 'workflows', 'ci.yml'), 'utf8')

  assert.deepEqual({
    usesExternalServer: /QNI_WEB_EXTERNAL_SERVER=1/.test(source),
    pollsReadiness: /Timed out waiting for static web server/.test(source),
  }, {
    usesExternalServer: true,
    pollsReadiness: true,
  })
})
