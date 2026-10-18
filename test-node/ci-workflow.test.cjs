const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

test('CI runs all four Playwright shards', async () => {
  const source = await fs.readFile(path.join(__dirname, '..', '.github', 'workflows', 'ci.yml'), 'utf8')

  assert.deepEqual({
    shardsPlaywright: /pnpm exec playwright test --shard=\$\{\{ matrix\.shard \}\}\/4/.test(source),
    matrix: /shard: \[1, 2, 3, 4\]/.test(source),
  }, {
    shardsPlaywright: true,
    matrix: true,
  })
})

test('CI web aggregator needs Playwright shards', async () => {
  const source = await fs.readFile(path.join(__dirname, '..', '.github', 'workflows', 'ci.yml'), 'utf8')

  assert.match(source, /\n  web:\n\s+needs: \[web-preflight, web-build, web-check, web-playwright\]/)
})

test('CI web aggregator rejects unsuccessful Playwright shards', async () => {
  const source = await fs.readFile(path.join(__dirname, '..', '.github', 'workflows', 'ci.yml'), 'utf8')

  assert.match(source, /test "\$PLAYWRIGHT_RESULT" = success/)
})

test('CI web aggregator needs web-preflight', async () => {
  const source = await fs.readFile(path.join(__dirname, '..', '.github', 'workflows', 'ci.yml'), 'utf8')
  const web = source.match(/^  web:\n[\s\S]*?(?=^  \S|$(?![\s\S]))/m)?.[0] ?? ''

  assert.match(web, /^    needs: \[[^\]\n]*\bweb-preflight\b[^\]\n]*\]/m)
})

test('CI web aggregator rejects unsuccessful web-preflight results', async () => {
  const source = await fs.readFile(path.join(__dirname, '..', '.github', 'workflows', 'ci.yml'), 'utf8')
  const web = source.match(/^  web:\n[\s\S]*?(?=^  \S|$(?![\s\S]))/m)?.[0] ?? ''

  assert.match(web, /env:\n(?:          [^\n]*\n)*?          ([A-Z_]+): \$\{\{ needs\.web-preflight\.result \}\}\n[\s\S]*?        run: \|\n(?:          [^\n]*\n)*?          test "\$\1" = success(?:\n|$)/)
})

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
